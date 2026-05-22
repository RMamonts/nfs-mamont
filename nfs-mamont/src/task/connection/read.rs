use std::io;
use std::net::SocketAddr;
use std::sync::Arc;

use monoio::net::tcp::TcpOwnedReadHalf;
use async_channel;
use tracing::{debug, error};

use async_channel::Sender;

use crate::allocator::Allocator;
use crate::mount::MountRes;
use crate::nlm::NlmRes;
use crate::parser::parser_struct::RpcParser;
use crate::parser::{
    ArgWrapper, ErrorWrapper, MountArgWrapper, MountArguments, NfsArgWrapper, NfsArguments,
    NlmArgWrapper, NlmArguments, ProcArguments,
};
use crate::task::global::mount::MountCommand;
use crate::task::global::nlm::NlmCommand;
use crate::task::{ProcReply, ProcResult};
use crate::vfs::NfsRes;

pub struct ReadTask<A: Allocator + Send + Sync + 'static> {
    readhalf: TcpOwnedReadHalf,
    client_addr: SocketAddr,
    mount_sender: async_channel::Sender<MountCommand>,
    nlm_sender: async_channel::Sender<NlmCommand>,
    result_sender: async_channel::Sender<ProcReply>,
    allocator: Arc<A>,
    pool_sender: Sender<(NfsArgWrapper, async_channel::Sender<ProcReply>)>,
}

impl<A: Allocator + Send + Sync + 'static> ReadTask<A> {
    pub fn new(
        readhalf: TcpOwnedReadHalf,
        client_addr: SocketAddr,
        mount_sender: async_channel::Sender<MountCommand>,
        nlm_sender: async_channel::Sender<NlmCommand>,
        result_sender: async_channel::Sender<ProcReply>,
        allocator: Arc<A>,
        pool_sender: Sender<(NfsArgWrapper, async_channel::Sender<ProcReply>)>,
    ) -> Self {
        Self {
            readhalf,
            client_addr,
            mount_sender,
            nlm_sender,
            result_sender,
            allocator,
            pool_sender,
        }
    }

    pub fn spawn(self) {
        monoio::spawn(async move { self.run().await });
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

                    if self.result_sender.send(result).await.is_err() {
                        return send_broken_pipe(&self.result_sender, header.xid, "");
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

                    if self.result_sender.send(result).await.is_err() {
                        return send_broken_pipe(&self.result_sender, header.xid, "");
                    }
                }

                Ok(ArgWrapper { proc: ProcArguments::Nfs3(proc), header }) => {
                    let xid = header.xid;
                    debug!(client=%self.client_addr, xid, program="NFS", proc="NON_NULL", "rpc dispatch");
                    let command = NfsArgWrapper { header, proc };

                    if self.pool_sender.send((command, self.result_sender.clone())).await.is_err() {
                        return send_broken_pipe(&self.result_sender, xid, "");
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

                    if self.result_sender.send(result).await.is_err() {
                        return send_broken_pipe(&self.result_sender, xid, "");
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
                    if self.mount_sender.send(command).await.is_err() {
                        return send_broken_pipe(&self.result_sender, xid, "");
                    }
                }

                Ok(ArgWrapper { proc: ProcArguments::Nlm4(proc), header }) => {
                    let xid = header.xid;
                    debug!(client=%self.client_addr, xid=header.xid, program="NLM", proc="NON_NULL", "rpc dispatch");
                    let command = NlmCommand {
                        result_tx: self.result_sender.clone(),
                        args: NlmArgWrapper { header, proc },
                    };

                    if self.nlm_sender.send(command).await.is_err() {
                        return send_broken_pipe(&self.result_sender, xid, "");
                    }
                }

                Err(ErrorWrapper { xid: Some(xid), error }) => {
                    error!(client=%self.client_addr, xid, error=?error, "rpc parse error");
                    let result = ProcReply { xid, proc_result: Err(error) };
                    if self.result_sender.send(result).await.is_err() {
                        return send_broken_pipe(&self.result_sender, xid, "");
                    }
                }

                Err(ErrorWrapper { xid: None, .. }) => {
                    error!(client=%self.client_addr, "rpc parse error: xid=<none>");
                    return Err(io::Error::from(io::ErrorKind::Other));
                }
            }
        }
    }
}

fn send_broken_pipe(
    sender: &async_channel::Sender<ProcReply>,
    xid: u32,
    _err: impl std::fmt::Display,
) -> io::Result<()> {
    Err(io::Error::new(io::ErrorKind::BrokenPipe, "channel closed"))
}
