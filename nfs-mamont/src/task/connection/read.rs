use std::io;
use std::net::SocketAddr;
use std::sync::Arc;

use tokio::net::tcp::OwnedReadHalf;
use tracing::{debug, error};

use async_channel::Sender;

use crate::allocator::{Allocator, Buffer};
use crate::mount::MountRes;
use crate::nlm::NlmRes;
use crate::parser::parser_struct::RpcParser;
use crate::parser::{
    ArgWrapper, ErrorWrapper, MountArgWrapper, MountArguments, NfsArgWrapper, NfsArguments,
    NlmArgWrapper, NlmArguments, ProcArguments,
};
use crate::rpc::Error;
use crate::task::global::mount::MountCommand;
use crate::task::global::nlm::NlmCommand;
use crate::task::{ProcCall, ProcReply, ProcResult};
use crate::vfs::NfsRes;

/// Wrapper for NLM, NFS, mount command senders.
pub struct CommandSenders<B: Buffer + 'static> {
    /// To send messages into mount task.
    mount_sender: Sender<MountCommand<B>>,
    /// To send command into nlm task.
    nlm_sender: Sender<NlmCommand<B>>,
    /// To pass (nfs_3_cmd, tx) into vfs task, so vfs task can send result back to write task.
    pool_sender: Sender<(NfsArgWrapper<B>, Sender<ProcReply<B>>)>,
    /// To pass into mount task as part of message,
    /// so mount task can send result back to write task
    /// and to bypass vfs with null procedure.
    result_sender: Sender<ProcReply<B>>,
    /// To pass into nlm task as part of message,
    /// so nlm task can send result back to write task.
    message_sender: Sender<ProcCall>,
}

impl<B: Buffer + 'static> CommandSenders<B> {
    pub fn new(
        mount_sender: Sender<MountCommand<B>>,
        nlm_sender: Sender<NlmCommand<B>>,
        pool_sender: Sender<(NfsArgWrapper<B>, Sender<ProcReply<B>>)>,
        result_sender: Sender<ProcReply<B>>,
        message_sender: Sender<ProcCall>,
    ) -> Self {
        Self { mount_sender, nlm_sender, pool_sender, result_sender, message_sender }
    }
}

/// Reads RPC commands from a network connection, parses them,
/// and forwards to [`super::super::global::vfs::VfsPool`] or other global tasks.
pub struct ReadTask<
    A: Allocator + Send + Sync + 'static,
    B: Buffer + 'static = <A as Allocator>::Buffer,
> {
    readhalf: OwnedReadHalf,
    client_addr: SocketAddr,
    command_senders: CommandSenders<B>,
    allocator: Arc<A>,
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
    ) -> Self {
        Self { readhalf, client_addr, command_senders, allocator }
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
        let mut parser = RpcParser::new(self.readhalf, self.allocator);

        loop {
            match parser.next_message().await {
                Ok(ArgWrapper { proc: ProcArguments::Nfs3(proc), header })
                    if matches!(*proc, NfsArguments::Null) =>
                {
                    debug!(client=%self.client_addr, xid=header.xid, program="NFS", proc="NULL", "rpc dispatch");
                    let result = ProcReply {
                        xid: header.xid,
                        proc_result: Ok(ProcResult::Nfs3(Box::new(NfsRes::Null))),
                    };

                    if let Err(err) = self.command_senders.result_sender.send(result).await {
                        return send_broken_pipe(
                            &self.command_senders.result_sender,
                            header.xid,
                            err,
                        )
                        .await;
                    }
                }

                Ok(ArgWrapper { proc: ProcArguments::Nlm4(proc), header })
                    if matches!(*proc, NlmArguments::Null) =>
                {
                    debug!(client=%self.client_addr, xid=header.xid, program="NLM", proc="NULL", "rpc dispatch");
                    let result = ProcReply {
                        xid: header.xid,
                        proc_result: Ok(ProcResult::Nlm4(Box::new(NlmRes::Null))),
                    };

                    if let Err(err) = self.command_senders.result_sender.send(result).await {
                        return send_broken_pipe(
                            &self.command_senders.result_sender,
                            header.xid,
                            err,
                        )
                        .await;
                    }
                }

                Ok(ArgWrapper { proc: ProcArguments::Nfs3(proc), header }) => {
                    let xid = header.xid;
                    debug!(client=%self.client_addr, xid, program="NFS", proc="NON_NULL", "rpc dispatch");
                    let command = NfsArgWrapper { header, proc };

                    if let Err(err) = self
                        .command_senders
                        .pool_sender
                        .send((command, self.command_senders.result_sender.clone()))
                        .await
                    {
                        return send_broken_pipe(&self.command_senders.result_sender, xid, err)
                            .await;
                    }
                }

                Ok(ArgWrapper { proc: ProcArguments::Mount(proc), header })
                    if matches!(*proc, MountArguments::Null) =>
                {
                    let xid = header.xid;
                    debug!(client=%self.client_addr, xid, program="MOUNT", proc="NULL", "rpc dispatch");

                    let result = ProcReply {
                        xid: header.xid,
                        proc_result: Ok(ProcResult::Mount(Box::new(MountRes::Null))),
                    };

                    if let Err(err) = self.command_senders.result_sender.send(result).await {
                        return send_broken_pipe(&self.command_senders.result_sender, xid, err)
                            .await;
                    }
                }

                Ok(ArgWrapper { proc: ProcArguments::Mount(proc), header }) => {
                    let xid = header.xid;
                    debug!(client=%self.client_addr, xid, program="MOUNT", proc="NON_NULL", "rpc dispatch");
                    let command = MountCommand {
                        result_tx: self.command_senders.result_sender.clone(),
                        args: MountArgWrapper { header, proc },
                        client_addr: self.client_addr,
                    };
                    if let Err(err) = self.command_senders.mount_sender.send(command).await {
                        return send_broken_pipe(&self.command_senders.result_sender, xid, err)
                            .await;
                    }
                }

                Ok(ArgWrapper { proc: ProcArguments::Nlm4(proc), header }) => {
                    let xid = header.xid;
                    debug!(client=%self.client_addr, xid=header.xid, program="NLM", proc="NON_NULL", "rpc dispatch");
                    let command = NlmCommand {
                        result_sender: self.command_senders.result_sender.clone(),
                        message_sender: self.command_senders.message_sender.clone(),
                        args: NlmArgWrapper { header, proc },
                    };

                    if let Err(err) = self.command_senders.nlm_sender.send(command).await {
                        return send_broken_pipe(&self.command_senders.result_sender, xid, err)
                            .await;
                    }
                }

                Err(ErrorWrapper { xid: Some(xid), error }) => {
                    error!(client=%self.client_addr, xid, error=?error, "rpc parse error");
                    let result = ProcReply { xid, proc_result: Err(error) };
                    if let Err(err) = self.command_senders.result_sender.send(result).await {
                        return send_broken_pipe(&self.command_senders.result_sender, xid, err)
                            .await;
                    }
                }

                // specific case when we couldn't parser xid, which means that we can't send reply
                Err(ErrorWrapper { xid: None, .. }) => {
                    error!(client=%self.client_addr, "rpc parse error: xid=<none>");
                    return Err(io::Error::from(io::ErrorKind::Other));
                }
            }
        }
    }
}

async fn send_broken_pipe<B: Buffer + 'static>(
    sender: &Sender<ProcReply<B>>,
    xid: u32,
    err: impl std::fmt::Display,
) -> io::Result<()> {
    sender
        .send(ProcReply {
            xid,
            proc_result: Err(Error::IO(io::Error::new(io::ErrorKind::BrokenPipe, err.to_string()))),
        })
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::BrokenPipe, e))
}
