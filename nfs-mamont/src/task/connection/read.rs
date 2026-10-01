use std::io;
use std::net::SocketAddr;
use std::sync::Arc;

use tokio::net::tcp::OwnedReadHalf;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
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
use crate::task::{ProcReply, ProcResult};
use crate::vfs::NfsRes;

use super::ConnectionContext;

/// Reads RPC commands from a network connection, parses them,
/// and forwards to [`super::super::global::vfs::VfsTask`] or other global tasks.
pub struct ReadTask<A: Allocator + Send + Sync + 'static, B: Buffer = <A as Allocator>::Buffer> {
    readhalf: OwnedReadHalf,
    client_addr: SocketAddr,
    // to send messages into mount task
    mount_sender: Sender<MountCommand<B>>,
    // to send messages into nlm task
    nlm_sender: Sender<NlmCommand<B>>,
    // to pass into mount task as part of message,
    // so mount task can send result back to write task
    // and
    // to bypass vfs with null procedure
    result_sender: Sender<ProcReply<B>>,
    allocator: Arc<A>,
    // to pass (nfs_3_cmd, tx) into vfs task, so vfs task can send result back to write task
    pool_sender: Sender<(NfsArgWrapper<B>, Sender<ProcReply<B>>)>,
    // cancelled when the server stops, so the task stops reading new requests
    shutdown: CancellationToken,
}

impl<A, B> ReadTask<A, B>
where
    A: Allocator<Buffer = B> + Send + Sync + 'static,
    B: Buffer + 'static,
{
    /// Creates new instance of [`ReadTask`] dispatching requests to the global tasks
    /// of `context`.
    pub fn new(
        readhalf: OwnedReadHalf,
        client_addr: SocketAddr,
        result_sender: Sender<ProcReply<B>>,
        context: &ConnectionContext<A>,
    ) -> Self {
        Self {
            readhalf,
            client_addr,
            mount_sender: context.mount_sender.clone(),
            nlm_sender: context.nlm_sender.clone(),
            result_sender,
            allocator: Arc::clone(&context.allocator),
            pool_sender: context.vfs_sender.clone(),
            shutdown: context.shutdown.clone(),
        }
    }

    /// Spawns a [`ReadTask`] that reads commands from a socket into `tasks`.
    ///
    /// # Panics
    ///
    /// If called outside of tokio runtime context.
    pub fn spawn(self, tasks: &mut JoinSet<()>)
    where
        B: 'static,
    {
        tasks.spawn(async move {
            let _ = self.run().await;
        });
    }

    async fn run(self) -> io::Result<()> {
        let mut parser = RpcParser::new(self.readhalf, self.allocator);
        // Registered once per connection, so waiting on it costs nothing per message.
        let shutdown = self.shutdown.cancelled();
        tokio::pin!(shutdown);

        loop {
            let message = tokio::select! {
                // Checked first, so a busy connection cannot delay the shutdown.
                biased;
                _ = &mut shutdown => {
                    debug!(client=%self.client_addr, "read task: server is shutting down");
                    return Ok(());
                }
                message = parser.next_message() => message,
            };

            match message {
                Ok(ArgWrapper { proc: ProcArguments::Nfs3(proc), header })
                    if matches!(*proc, NfsArguments::Null) =>
                {
                    debug!(client=%self.client_addr, xid=header.xid, program="NFS", proc="NULL", "rpc dispatch");
                    let result = ProcReply {
                        xid: header.xid,
                        proc_result: Ok(ProcResult::Nfs3(Box::new(NfsRes::Null))),
                    };

                    if let Err(err) = self.result_sender.send(result).await {
                        return send_broken_pipe(&self.result_sender, header.xid, err).await;
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

                    if let Err(err) = self.result_sender.send(result).await {
                        return send_broken_pipe(&self.result_sender, header.xid, err).await;
                    }
                }

                Ok(ArgWrapper { proc: ProcArguments::Nfs3(proc), header }) => {
                    let xid = header.xid;
                    debug!(client=%self.client_addr, xid, program="NFS", proc="NON_NULL", "rpc dispatch");
                    let command = NfsArgWrapper { header, proc };

                    if let Err(err) =
                        self.pool_sender.send((command, self.result_sender.clone())).await
                    {
                        return send_broken_pipe(&self.result_sender, xid, err).await;
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

                    if let Err(err) = self.result_sender.send(result).await {
                        return send_broken_pipe(&self.result_sender, xid, err).await;
                    }
                }

                Ok(ArgWrapper { proc: ProcArguments::Mount(proc), header }) => {
                    let xid = header.xid;
                    debug!(client=%self.client_addr, xid, program="MOUNT", proc="NON_NULL", "rpc dispatch");
                    let command = MountCommand {
                        result_tx: self.result_sender.clone(),
                        args: MountArgWrapper { header, proc },
                        client_addr: self.client_addr,
                    };
                    if let Err(err) = self.mount_sender.send(command).await {
                        return send_broken_pipe(&self.result_sender, xid, err).await;
                    }
                }

                Ok(ArgWrapper { proc: ProcArguments::Nlm4(proc), header }) => {
                    let xid = header.xid;
                    debug!(client=%self.client_addr, xid=header.xid, program="NLM", proc="NON_NULL", "rpc dispatch");
                    let command = NlmCommand {
                        result_tx: self.result_sender.clone(),
                        args: NlmArgWrapper { header, proc },
                    };

                    if let Err(err) = self.nlm_sender.send(command).await {
                        return send_broken_pipe(&self.result_sender, xid, err).await;
                    }
                }

                Err(ErrorWrapper { xid: Some(xid), error }) => {
                    error!(client=%self.client_addr, xid, error=?error, "rpc parse error");
                    let result = ProcReply { xid, proc_result: Err(error) };
                    if let Err(err) = self.result_sender.send(result).await {
                        return send_broken_pipe(&self.result_sender, xid, err).await;
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
