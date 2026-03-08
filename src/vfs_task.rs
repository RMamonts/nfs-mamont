use std::collections::BTreeMap;
use std::io;
use std::time::Instant;

use tokio::sync::mpsc::{Receiver, Sender};
use tokio::task::JoinHandle;
use tokio::task::JoinSet;
use tracing::{debug, info};

use crate::mount;
use crate::parser::Arguments;
use crate::rpc::{ReplyEnvelope, RpcCommand, ServerContext, SharedVfs};
use crate::serializer::{serialize_reply, MountRes, NfsRes, ProcResult};

/// Process RPC commands, sends operation results to [`crate::write_task::WriteTask`].
pub struct VfsTask {
    command_receiver: Receiver<RpcCommand>,
    result_sender: Sender<ReplyEnvelope>,
    server_context: ServerContext,
}

impl VfsTask {
    /// Creates new instance of [`VfsTask`].
    pub fn new(
        command_receiver: Receiver<RpcCommand>,
        result_sender: Sender<ReplyEnvelope>,
        server_context: ServerContext,
    ) -> Self {
        Self { command_receiver, result_sender, server_context }
    }

    /// Spawns a [`VfsTask`].
    ///
    /// # Panics
    ///
    /// If called outside of tokio runtime context.
    pub fn spawn(self) -> JoinHandle<()> {
        tokio::spawn(async move { self.run().await })
    }

    async fn run(mut self) {
        let max_in_flight = self.server_context.settings().max_in_flight_requests().get();
        let mut receiver_closed = false;
        let mut next_sequence = 0_u64;
        let mut next_reply_sequence = 0_u64;
        let mut pending = BTreeMap::new();
        let mut in_flight = JoinSet::new();

        loop {
            self.flush_ready_replies(&mut pending, &mut next_reply_sequence).await;

            if receiver_closed && in_flight.is_empty() && pending.is_empty() {
                info!("vfs task finished");
                break;
            }

            if receiver_closed || in_flight.len() >= max_in_flight {
                if let Some(Ok((sequence, result))) = in_flight.join_next().await {
                    pending.insert(sequence, result);
                }
                continue;
            }

            tokio::select! {
                maybe_command = self.command_receiver.recv() => {
                    match maybe_command {
                        Some(command) => {
                            let sequence = next_sequence;
                            next_sequence += 1;
                            let server_context = self.server_context.clone();
                            let span = command.context.span.clone();
                            let result_queue_depth = queue_depth(&self.result_sender);
                            debug!(
                                parent: &span,
                                sequence,
                                result_queue_depth,
                                queue_wait_micros = command.context.received_at.elapsed().as_micros() as u64,
                                "queued rpc request for dispatch",
                            );
                            in_flight.spawn(async move {
                                let xid = command.context.header.xid;
                                let received_at = command.context.received_at;
                                let dispatched_at = Instant::now();
                                let result = serialize_reply(
                                    xid,
                                    Self::dispatch_with_context(server_context, command).await,
                                )
                                .await;
                                debug!(
                                    parent: &span,
                                    sequence,
                                    dispatch_micros = dispatched_at.elapsed().as_micros() as u64,
                                    "completed rpc dispatch",
                                );
                                (
                                    sequence,
                                    ReplyEnvelope::new(result, span, received_at, Some(dispatched_at)),
                                )
                            });
                        }
                        None => receiver_closed = true,
                    }
                }
                joined = in_flight.join_next(), if !in_flight.is_empty() => {
                    if let Some(Ok((sequence, result))) = joined {
                        pending.insert(sequence, result);
                    }
                }
            }
        }
    }

    async fn flush_ready_replies(
        &self,
        pending: &mut BTreeMap<u64, ReplyEnvelope>,
        next_reply_sequence: &mut u64,
    ) {
        while let Some(reply) = pending.remove(next_reply_sequence) {
            debug!(
                parent: &reply.span,
                sequence = *next_reply_sequence,
                writer_queue_depth = queue_depth(&self.result_sender),
                total_elapsed_micros = reply.received_at.elapsed().as_micros() as u64,
                "forwarding rpc reply to writer",
            );
            if self.result_sender.send(reply).await.is_err() {
                break;
            }
            *next_reply_sequence += 1;
        }
    }

    async fn dispatch_with_context(
        server_context: ServerContext,
        command: RpcCommand,
    ) -> Result<ProcResult, crate::rpc::Error> {
        debug!(
            parent: &command.context.span,
            command.context.header.program,
            command.context.header.version,
            command.context.header.procedure,
            auth = ?command.context.connection.auth(),
            "dispatching rpc request",
        );
        match *command.arguments {
            Arguments::Null => Ok(match command.context.header.program {
                crate::nfsv3::NFS_PROGRAM => ProcResult::Nfs3(NfsRes::Null),
                crate::mount::MOUNT_PROGRAM => ProcResult::Mount(MountRes::Null),
                _ => return Err(crate::rpc::Error::ProgramMismatch),
            }),
            other => {
                Self::dispatch_non_null_with_context(
                    server_context,
                    other,
                    &command.context.connection,
                )
                .await
            }
        }
    }

    async fn dispatch_non_null_with_context(
        server_context: ServerContext,
        arguments: Arguments,
        connection: &crate::rpc::ConnectionContext,
    ) -> Result<ProcResult, crate::rpc::Error> {
        match mount::MountRequest::try_from(arguments) {
            Ok(request) => {
                mount::MountService::new(connection, &server_context).dispatch(request).await
            }
            Err(other) => Self::dispatch_nfs_with_context(server_context, other).await,
        }
    }

    async fn dispatch_nfs_with_context(
        server_context: ServerContext,
        arguments: Arguments,
    ) -> Result<ProcResult, crate::rpc::Error> {
        macro_rules! dispatch_backend {
            ($args:ident, $method:ident, $variant:ident) => {
                Ok(ProcResult::Nfs3(NfsRes::$variant(
                    Self::backend(&server_context)?.$method($args).await,
                )))
            };
        }

        match arguments {
            Arguments::GetAttr(args) => dispatch_backend!(args, get_attr, GetAttr),
            Arguments::SetAttr(args) => dispatch_backend!(args, set_attr, SetAttr),
            Arguments::LookUp(args) => dispatch_backend!(args, lookup, LookUp),
            Arguments::Access(args) => dispatch_backend!(args, access, Access),
            Arguments::ReadLink(args) => dispatch_backend!(args, read_link, ReadLink),
            Arguments::Read(args) => dispatch_backend!(args, read, Read),
            Arguments::Write(args) => dispatch_backend!(args, write, Write),
            Arguments::Create(args) => dispatch_backend!(args, create, Create),
            Arguments::MkDir(args) => dispatch_backend!(args, mk_dir, MkDir),
            Arguments::SymLink(args) => dispatch_backend!(args, symlink, SymLink),
            Arguments::MkNod(args) => dispatch_backend!(args, mk_node, MkNod),
            Arguments::Remove(args) => dispatch_backend!(args, remove, Remove),
            Arguments::RmDir(args) => dispatch_backend!(args, rm_dir, RmDir),
            Arguments::Rename(args) => dispatch_backend!(args, rename, Rename),
            Arguments::Link(args) => dispatch_backend!(args, link, Link),
            Arguments::ReadDir(args) => dispatch_backend!(args, read_dir, ReadDir),
            Arguments::ReadDirPlus(args) => dispatch_backend!(args, read_dir_plus, ReadDirPlus),
            Arguments::FsStat(args) => dispatch_backend!(args, fs_stat, FsStat),
            Arguments::FsInfo(args) => dispatch_backend!(args, fs_info, FsInfo),
            Arguments::PathConf(args) => dispatch_backend!(args, path_conf, PathConf),
            Arguments::Commit(args) => dispatch_backend!(args, commit, Commit),
            _ => Err(crate::rpc::Error::ProcedureMismatch),
        }
    }

    fn backend(server_context: &ServerContext) -> Result<&SharedVfs, crate::rpc::Error> {
        server_context.backend().ok_or_else(|| {
            crate::rpc::Error::IO(io::Error::other("server backend is not configured"))
        })
    }
}

fn queue_depth<T>(sender: &Sender<T>) -> usize {
    sender.max_capacity().saturating_sub(sender.capacity())
}
