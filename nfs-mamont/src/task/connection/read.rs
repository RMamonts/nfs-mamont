use std::io;
use std::sync::Arc;

use tokio::net::tcp::OwnedReadHalf;
use tracing::{debug, error, warn};

use async_channel::{SendError, Sender};

use crate::allocator::{Allocator, Buffer};
use crate::connection_id::ConnectionId;
use crate::metric::{NfsMetrics, TaskKind, Timer};
use crate::mount::MountRes;
use crate::nlm::NlmRes;
use crate::parser::parser_struct::RpcParser;
use crate::parser::{
    ArgWrapper, MountArgWrapper, MountArguments, NfsArgWrapper, NfsArguments, NlmArgWrapper,
    NlmArguments, ProcArguments,
};
use crate::rpc::{Error, ErrorWrapper};
use crate::task::connection::limit::Budget;
use crate::task::global::mount::MountCommand;
use crate::task::global::nlm::nlm_task::NlmCommand;
use crate::task::global::vfs::{VfsCommand, VfsCommandSender};
use crate::task::{ProcReply, ProcResult};
use crate::vfs::NfsRes;

/// Wrapper for NLM, NFS, mount command senders.
pub struct CommandSenders<B: Buffer + 'static> {
    /// To send messages into mount task.
    mount_sender: Sender<MountCommand<B>>,
    /// To send command into nlm task.
    nlm_sender: Sender<NlmCommand<B>>,
    /// To pass command into vfs task.
    pool_sender: VfsCommandSender<B>,
}

impl<B: Buffer + 'static> CommandSenders<B> {
    pub fn new(
        mount_sender: Sender<MountCommand<B>>,
        nlm_sender: Sender<NlmCommand<B>>,
        pool_sender: VfsCommandSender<B>,
    ) -> Self {
        Self { mount_sender, nlm_sender, pool_sender }
    }
}

/// Reads RPC commands from a network connection, parses them,
/// and forwards to [`super::super::global::vfs::VfsTask`] or other global tasks.
pub struct ReadTask<
    A: Allocator + Send + Sync + 'static,
    B: Buffer + 'static = <A as Allocator>::Buffer,
> {
    readhalf: OwnedReadHalf,
    client_id: ConnectionId,
    command_senders: CommandSenders<B>,
    allocator: Arc<A>,
    metrics: Arc<NfsMetrics>,
    /// In-flight budget of the connection, see the [`super`] module docs.
    budget: Budget<B>,
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
        command_senders: CommandSenders<B>,
        metrics: Arc<NfsMetrics>,
        budget: Budget<B>,
    ) -> Self {
        Self { readhalf, client_id, allocator, command_senders, metrics, budget }
    }

    /// Spawns a [`ReadTask`]  that reads commands from a socket.
    ///
    /// # Panics
    ///
    /// If called outside of tokio runtime context.
    pub fn spawn(self)
    where
        B: 'static,
    {
        let metrics = Arc::clone(&self.metrics);
        tokio::spawn(metrics.instrument(TaskKind::Read, self.run()));
    }

    async fn run(mut self) -> io::Result<()> {
        let (readhalf, io) = self.metrics.io_timer(self.readhalf);
        let mut parser = RpcParser::new(readhalf, self.allocator);

        loop {
            // Taken before the request is read: while the connection has `max_in_flight`
            // unanswered requests the socket stays unread and TCP flow control holds the
            // client back.
            let result_tx = self.budget.acquire().await;
            let message =
                self.metrics.read_call(&io, parser.next_message(), |message| match message {
                    Ok(ArgWrapper { proc: ProcArguments::Nfs3(args), .. }) => {
                        Some(args.procedure())
                    }

                    _ => None,
                });
            match message.await {
                Ok(ArgWrapper { proc: ProcArguments::Nfs3(proc), header })
                    if matches!(*proc, NfsArguments::Null) =>
                {
                    debug!(client=%self.client_id.into_inner(), xid=header.xid, program="NFS", proc="NULL", "rpc dispatch");
                    let result = ProcReply {
                        xid: header.xid,
                        proc_result: Ok(ProcResult::Nfs3(Box::new(NfsRes::Null))),
                        timer: self.metrics.start(),
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
                        timer: Timer::NONE,
                    };

                    result_tx.send(result).await?;
                }

                Ok(ArgWrapper { proc: ProcArguments::Nfs3(proc), header }) => {
                    let timer = self.metrics.start();
                    let xid = header.xid;
                    debug!(client=%self.client_id.into_inner(), xid, program="NFS", proc="NON_NULL", "rpc dispatch");
                    let command =
                        VfsCommand { args: NfsArgWrapper { header, proc }, result_tx, timer };

                    if let Err(SendError(command)) =
                        self.command_senders.pool_sender.send(command).await
                    {
                        return command.result_tx.broken_pipe(xid).await;
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
                        timer: Timer::NONE,
                    };

                    result_tx.send(result).await?;
                }

                Ok(ArgWrapper { proc: ProcArguments::Mount(proc), header }) => {
                    let xid = header.xid;
                    debug!(client=%self.client_id.into_inner(), xid, program="MOUNT", proc="NON_NULL", "rpc dispatch");
                    let command = MountCommand {
                        result_tx,
                        args: MountArgWrapper { header, proc },
                        connection_id: self.client_id,
                    };
                    if let Err(SendError(command)) =
                        self.command_senders.mount_sender.send(command).await
                    {
                        return command.result_tx.broken_pipe(xid).await;
                    }
                }

                Ok(ArgWrapper { proc: ProcArguments::Nlm4(proc), header }) => {
                    let xid = header.xid;
                    debug!(client=%self.client_id.into_inner(), xid=header.xid, program="NLM", proc="NON_NULL", "rpc dispatch");
                    let command = NlmCommand {
                        result_sender: result_tx,
                        args: NlmArgWrapper { header, proc },
                        connection_id: self.client_id,
                    };

                    if let Err(SendError(command)) =
                        self.command_senders.nlm_sender.send(command).await
                    {
                        return command.result_sender.broken_pipe(xid).await;
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
                    let result = ProcReply { xid, proc_result: Err(error), timer: Timer::NONE };
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
