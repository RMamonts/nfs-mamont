use std::io;
use std::sync::Arc;

use tokio::net::tcp::OwnedReadHalf;
use tokio::task::AbortHandle;
use tracing::{debug, error, warn};

use crate::allocator::{Allocator, Buffer};
use crate::connection_id::ConnectionId;
use crate::limits::connection::{ConnectionLimits, Expiry};
use crate::mount::MountRes;
use crate::nlm::NlmRes;
use crate::parser::parser_struct::RpcParser;
use crate::parser::{
    ArgWrapper, MountArgWrapper, MountArguments, NfsArgWrapper, NfsArguments, NlmArgWrapper,
    NlmArguments, ProcArguments,
};
use crate::rpc::{Error, ErrorWrapper};
use crate::task::global::manager::{ManagerCommand, ManagerCommandSender};
use crate::task::global::mount::MountCommand;
use crate::task::global::nlm::nlm_task::NlmCommand;
use crate::task::global::vfs::VfsCommand;
use crate::task::{ProcReply, ProcResult};
use crate::vfs::nfsv3::NfsRes;

/// Reads RPC commands from a network connection, parses them,
/// and forwards to [`super::super::global::manager::ManagerTask`].
pub struct ReadTask<
    A: Allocator + Send + Sync + 'static,
    B: Buffer + 'static = <A as Allocator>::Buffer,
> {
    readhalf: OwnedReadHalf,
    client_id: ConnectionId,
    manager_sender: ManagerCommandSender<B>,
    allocator: Arc<A>,
    /// Limits of the connection, see [`crate::limits`].
    limits: ConnectionLimits<B>,
}

impl<A, B> ReadTask<A, B>
where
    A: Allocator<Buffer = B> + Send + Sync + 'static,
    B: Buffer + 'static,
{
    /// Creates new instance of [`ReadTask`]
    pub fn new(
        readhalf: OwnedReadHalf,
        client_id: ConnectionId,
        allocator: Arc<A>,
        manager_sender: ManagerCommandSender<B>,
        limits: ConnectionLimits<B>,
    ) -> Self {
        Self { readhalf, client_id, manager_sender, allocator, limits }
    }

    /// Spawns a [`ReadTask`]  that reads commands from a socket.
    ///
    /// # Panics
    ///
    /// If called outside of tokio runtime context.
    pub fn spawn(self) -> AbortHandle
    where
        B: 'static,
    {
        tokio::spawn(async move { self.run().await }).abort_handle()
    }

    async fn run(mut self) -> io::Result<()> {
        let mut parser = RpcParser::new(self.readhalf, self.allocator);

        loop {
            // Taken before the request is read: while the connection has `max_in_flight`
            // unanswered requests the socket stays unread and TCP flow control holds the
            // client back.
            let mut result_tx = self.limits.acquire().await;

            let message = tokio::select! {
                biased;
                message = parser.next_message() => message,
                expiry = self.limits.expired() => {
                    let client = self.client_id.into_inner();
                    match expiry {
                        Expiry::Evicted => debug!(%client, "closing a connection that has sent no valid call"),
                        Expiry::Idle => debug!(%client, "closing an idle connection"),
                    }
                    return Ok(());
                }
            };
            if message.is_ok() {
                // The first valid call ends the probation.
                self.limits.verified();
            }

            match message {
                Ok(ArgWrapper { proc: ProcArguments::Nfs3(proc), header })
                    if matches!(*proc, NfsArguments::Null) =>
                {
                    debug!(client=%self.client_id.into_inner(), xid=header.xid, program="NFS", proc="NULL", "rpc dispatch");
                    let result = ProcReply {
                        xid: header.xid,
                        proc_result: Ok(ProcResult::Nfs3(Box::new(NfsRes::Null))),
                    };

                    result_tx.send(result).await?;
                }

                Ok(ArgWrapper { proc: ProcArguments::Nlm4(proc), header })
                    if matches!(*proc, NlmArguments::Null) =>
                {
                    debug!(client=%self.client_id.into_inner(), xid=header.xid, program="NLM", proc="NULL", "rpc dispatch");
                    let result = ProcReply {
                        xid: header.xid,
                        proc_result: Ok(ProcResult::Nlm4(Box::new(NlmRes::Null))),
                    };

                    result_tx.send(result).await?;
                }

                Ok(ArgWrapper { proc: ProcArguments::Nfs3(proc), header }) => {
                    let xid = header.xid;
                    debug!(client=%self.client_id.into_inner(), xid, program="NFS", proc="NON_NULL", "rpc dispatch");
                    self.limits.charge(&mut result_tx, &proc);
                    // Parsed before the manager's slot is taken, so that an idle
                    // connection holds no slot.
                    let command = ManagerCommand::Nfs(VfsCommand {
                        args: NfsArgWrapper { header, proc },
                        result_tx,
                    });
                    if let Err(command) = self.manager_sender.send(command).await {
                        return broken_pipe(command).await;
                    }
                }

                Ok(ArgWrapper { proc: ProcArguments::Mount(proc), header })
                    if matches!(*proc, MountArguments::Null) =>
                {
                    let xid = header.xid;
                    debug!(client=%self.client_id.into_inner(), xid, program="MOUNT", proc="NULL", "rpc dispatch");

                    let result = ProcReply {
                        xid: header.xid,
                        proc_result: Ok(ProcResult::Mount(Box::new(MountRes::Null))),
                    };

                    result_tx.send(result).await?;
                }

                Ok(ArgWrapper { proc: ProcArguments::Mount(proc), header }) => {
                    let xid = header.xid;
                    debug!(client=%self.client_id.into_inner(), xid, program="MOUNT", proc="NON_NULL", "rpc dispatch");
                    let command = ManagerCommand::Mount(MountCommand {
                        result_tx,
                        args: MountArgWrapper { header, proc },
                        connection_id: self.client_id,
                    });
                    if let Err(command) = self.manager_sender.send(command).await {
                        return broken_pipe(command).await;
                    }
                }

                Ok(ArgWrapper { proc: ProcArguments::Nlm4(proc), header }) => {
                    let xid = header.xid;
                    debug!(client=%self.client_id.into_inner(), xid, program="NLM", proc="NON_NULL", "rpc dispatch");
                    let command = ManagerCommand::Nlm(NlmCommand {
                        result_sender: result_tx,
                        args: NlmArgWrapper { header, proc },
                        connection_id: self.client_id,
                    });

                    if let Err(command) = self.manager_sender.send(command).await {
                        return broken_pipe(command).await;
                    }
                }

                // The record is not a CALL at all (a REPLY sent to the server, or a
                // bogus msg_type). RFC 5531 defines no reply to a reply, and answering
                // one with a reply of our own could ping-pong with a confused peer, so
                // the record is dropped silently.
                Err(ErrorWrapper { xid: Some(xid), error: Error::MessageTypeMismatch }) => {
                    warn!(client=%self.client_id.into_inner(), xid, "rpc record is not a CALL, ignoring");
                }

                Err(ErrorWrapper { xid: Some(xid), error }) => {
                    error!(client=%self.client_id.into_inner(), xid, error=?error, "rpc parse error");
                    let result = ProcReply { xid, proc_result: Err(error) };
                    result_tx.send(result).await?;
                }

                // specific case when we couldn't parser xid, which means that we can't send reply
                Err(ErrorWrapper { xid: None, .. }) => {
                    error!(client=%self.client_id.into_inner(), "rpc parse error: xid=<none>");
                    return Err(io::Error::from(io::ErrorKind::Other));
                }
            }
        }
    }
}

/// Fails the request with a `BrokenPipe` because the task manager is gone (the server is
/// shutting down); every command carries its own reply channel, whichever protocol it is.
async fn broken_pipe<B: Buffer>(command: ManagerCommand<B>) -> io::Result<()> {
    match command {
        ManagerCommand::Nfs(command) => {
            let xid = command.args.header.xid;
            command.result_tx.broken_pipe(xid).await
        }
        ManagerCommand::Mount(command) => {
            let xid = command.args.header.xid;
            command.result_tx.broken_pipe(xid).await
        }
        ManagerCommand::Nlm(command) => {
            let xid = command.args.header.xid;
            command.result_sender.broken_pipe(xid).await
        }
    }
}
