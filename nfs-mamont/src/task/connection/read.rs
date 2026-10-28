use std::io;
use std::net::SocketAddr;
use std::sync::Arc;

use async_channel::{SendError, Sender};
use tokio::net::tcp::OwnedReadHalf;
use tracing::{debug, error, warn};

use crate::allocator::{Allocator, Buffer};
use crate::mount::MountRes;
use crate::nlm::NlmRes;
use crate::parser::parser_struct::RpcParser;
use crate::parser::{
    ArgWrapper, MountArgWrapper, MountArguments, NfsArgWrapper, NfsArguments, NlmArgWrapper,
    NlmArguments, ProcArguments,
};
use crate::rpc::{Error, ErrorWrapper};
use crate::task::connection::limit::{Budget, Token};
use crate::task::global::mount::MountCommand;
use crate::task::global::nlm::nlm_task::NlmCommand;
use crate::task::global::vfs::VfsCommand;
use crate::task::{InFlightReply, ProcReply, ProcResult};
use crate::vfs::NfsRes;

/// Wrapper for NLM, NFS, mount command senders.
pub struct CommandSenders<B: Buffer + 'static> {
    /// To send messages into mount task.
    mount_sender: Sender<MountCommand<B>>,
    /// To send command into nlm task.
    nlm_sender: Sender<NlmCommand<B>>,
    /// To pass command into vfs task.
    pool_sender: Sender<VfsCommand<B>>,
    /// To pass into mount task as part of message,
    /// so mount task can send result back to write task
    /// and to bypass vfs with null procedure.
    result_sender: Sender<InFlightReply<B>>,
}

impl<B: Buffer + 'static> CommandSenders<B> {
    pub fn new(
        mount_sender: Sender<MountCommand<B>>,
        nlm_sender: Sender<NlmCommand<B>>,
        pool_sender: Sender<VfsCommand<B>>,
        result_sender: Sender<InFlightReply<B>>,
    ) -> Self {
        Self { mount_sender, nlm_sender, pool_sender, result_sender }
    }

    /// Queues `reply` for the write task together with the in-flight slot it frees.
    ///
    /// # Errors
    ///
    /// `BrokenPipe` when the write task is gone, i.e. the connection is closed.
    async fn reply(&self, reply: ProcReply<B>, token: Token) -> io::Result<()> {
        self.result_sender
            .send(InFlightReply { reply, token })
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "write task closed"))
    }

    /// Answers `xid` with a `BrokenPipe` error because the global task that should have
    /// served it is gone (the server is shutting down); the connection is done after it.
    async fn broken_pipe(&self, xid: u32, token: Token) -> io::Result<()> {
        let error = io::Error::new(io::ErrorKind::BrokenPipe, "server task closed");
        self.reply(ProcReply { xid, proc_result: Err(Error::IO(error)) }, token).await
    }
}

/// Reads RPC commands from a network connection, parses them,
/// and forwards to [`super::super::global::vfs::VfsTask`] or other global tasks.
pub struct ReadTask<
    A: Allocator + Send + Sync + 'static,
    B: Buffer + 'static = <A as Allocator>::Buffer,
> {
    readhalf: OwnedReadHalf,
    client_addr: SocketAddr,
    command_senders: CommandSenders<B>,
    allocator: Arc<A>,
    /// In-flight budget of the connection, see the [`super`] module docs.
    budget: Budget,
}

impl<A, B> ReadTask<A, B>
where
    A: Allocator<Buffer = B> + Send + Sync + 'static,
    B: Buffer + 'static,
{
    /// Creates new instance of [`ReadTask`]
    pub fn new(
        readhalf: OwnedReadHalf,
        client_addr: SocketAddr,
        allocator: Arc<A>,
        command_senders: CommandSenders<B>,
        budget: Budget,
    ) -> Self {
        Self { readhalf, client_addr, allocator, command_senders, budget }
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
        tokio::spawn(async move { self.run().await });
    }

    async fn run(self) -> io::Result<()> {
        let Self { readhalf, client_addr, command_senders, allocator, mut budget } = self;
        let mut parser = RpcParser::new(readhalf, allocator);

        loop {
            // Taken before the request is read: while the connection has `max_in_flight`
            // unanswered requests the socket stays unread and TCP flow control holds the
            // client back. The token returns to the write task inside the reply.
            let Some(token) = budget.acquire().await else {
                debug!(client=%client_addr, "write task is gone, closing the connection");
                return Ok(());
            };

            match parser.next_message().await {
                Ok(ArgWrapper { proc: ProcArguments::Nfs3(proc), header })
                    if matches!(*proc, NfsArguments::Null) =>
                {
                    debug!(client=%client_addr, xid=header.xid, program="NFS", proc="NULL", "rpc dispatch");
                    let reply = ProcReply {
                        xid: header.xid,
                        proc_result: Ok(ProcResult::Nfs3(Box::new(NfsRes::Null))),
                    };
                    command_senders.reply(reply, token).await?;
                }

                Ok(ArgWrapper { proc: ProcArguments::Nlm4(proc), header })
                    if matches!(*proc, NlmArguments::Null) =>
                {
                    debug!(client=%client_addr, xid=header.xid, program="NLM", proc="NULL", "rpc dispatch");
                    let reply = ProcReply {
                        xid: header.xid,
                        proc_result: Ok(ProcResult::Nlm4(Box::new(NlmRes::Null))),
                    };
                    command_senders.reply(reply, token).await?;
                }

                Ok(ArgWrapper { proc: ProcArguments::Nfs3(proc), header }) => {
                    debug!(client=%client_addr, xid=header.xid, program="NFS", proc="NON_NULL", "rpc dispatch");
                    let command = VfsCommand {
                        args: NfsArgWrapper { header, proc },
                        result_tx: command_senders.result_sender.clone(),
                        token,
                    };

                    if let Err(SendError(command)) = command_senders.pool_sender.send(command).await
                    {
                        return command_senders
                            .broken_pipe(command.args.header.xid, command.token)
                            .await;
                    }
                }

                Ok(ArgWrapper { proc: ProcArguments::Mount(proc), header })
                    if matches!(*proc, MountArguments::Null) =>
                {
                    debug!(client=%client_addr, xid=header.xid, program="MOUNT", proc="NULL", "rpc dispatch");
                    let reply = ProcReply {
                        xid: header.xid,
                        proc_result: Ok(ProcResult::Mount(Box::new(MountRes::Null))),
                    };
                    command_senders.reply(reply, token).await?;
                }

                Ok(ArgWrapper { proc: ProcArguments::Mount(proc), header }) => {
                    debug!(client=%client_addr, xid=header.xid, program="MOUNT", proc="NON_NULL", "rpc dispatch");
                    let command = MountCommand {
                        result_tx: command_senders.result_sender.clone(),
                        args: MountArgWrapper { header, proc },
                        client_addr,
                        token,
                    };

                    if let Err(SendError(command)) =
                        command_senders.mount_sender.send(command).await
                    {
                        return command_senders
                            .broken_pipe(command.args.header.xid, command.token)
                            .await;
                    }
                }

                Ok(ArgWrapper { proc: ProcArguments::Nlm4(proc), header }) => {
                    debug!(client=%client_addr, xid=header.xid, program="NLM", proc="NON_NULL", "rpc dispatch");
                    let command = NlmCommand {
                        result_sender: command_senders.result_sender.clone(),
                        args: NlmArgWrapper { header, proc },
                        token,
                    };

                    if let Err(SendError(command)) = command_senders.nlm_sender.send(command).await
                    {
                        return command_senders
                            .broken_pipe(command.args.header.xid, command.token)
                            .await;
                    }
                }

                // The record is not a CALL at all (a REPLY sent to the server, or a
                // bogus msg_type). RFC 5531 defines no reply to a reply, and answering
                // one with a reply of our own could ping-pong with a confused peer, so
                // the record is dropped silently and its in-flight slot is given back.
                Err(ErrorWrapper { xid: Some(xid), error: Error::MessageTypeMismatch }) => {
                    warn!(client=%client_addr, xid, "rpc record is not a CALL, ignoring");
                    budget.cancel(token);
                }

                Err(ErrorWrapper { xid: Some(xid), error }) => {
                    error!(client=%client_addr, xid, error=?error, "rpc parse error");
                    command_senders
                        .reply(ProcReply { xid, proc_result: Err(error) }, token)
                        .await?;
                }

                // specific case when we couldn't parser xid, which means that we can't send reply
                Err(ErrorWrapper { xid: None, .. }) => {
                    error!(client=%client_addr, "rpc parse error: xid=<none>");
                    return Err(io::Error::from(io::ErrorKind::Other));
                }
            }
        }
    }
}
