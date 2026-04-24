use std::io;
use std::net::SocketAddr;
use std::sync::Arc;

use async_channel::Sender;
use async_trait::async_trait;
use tokio_uring::net::TcpStream;
use tracing::{debug, error};

use crate::allocator::Impl;
use crate::mount::MountRes;
use crate::parser::parser_struct::RpcParser;
use crate::parser::read_buffer::ReadSource;
use crate::parser::{
    ArgWrapper, ErrorWrapper, MountArgWrapper, MountArguments, NfsArgWrapper, NfsArguments,
    ProcArguments,
};
use crate::rpc::Error;
use crate::task::global::mount::MountCommand;
use crate::task::{ProcReply, ProcResult};
use crate::vfs::NfsRes;

struct UringReadStream {
    socket: Arc<TcpStream>,
}

impl UringReadStream {
    fn new(socket: Arc<TcpStream>) -> Self {
        Self { socket }
    }
}

#[async_trait(?Send)]
impl ReadSource for UringReadStream {
    async fn read_into(&mut self, dest: &mut [u8]) -> io::Result<usize> {
        let (result, buf) = self.socket.read(vec![0u8; dest.len()]).await;
        let n = result?;
        dest[..n].copy_from_slice(&buf[..n]);
        Ok(n)
    }

    async fn read_exact_into(&mut self, dest: &mut [u8]) -> io::Result<()> {
        let mut total = 0;
        while total < dest.len() {
            let read_now = self.read_into(&mut dest[total..]).await?;
            if read_now == 0 {
                return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Connection closed"));
            }
            total += read_now;
        }
        Ok(())
    }
}

/// Reads RPC commands from a network connection, parses them,
/// and forwards to [`super::super::global::vfs::VfsPool`] or other global tasks.
pub struct ReadTask {
    socket: Arc<TcpStream>,
    client_addr: SocketAddr,
    // to send messages into mount task
    mount_sender: Sender<MountCommand>,
    // to pass into mount task as part of message,
    // so mount task can send result back to write task
    // and
    // to bypass vfs with null procedure
    result_sender: Sender<ProcReply>,
    allocator: Arc<Impl>,
    // to pass (nfs_3_cmd, tx) into vfs task, so vfs task can send result back to write task
    pool_sender: Sender<(NfsArgWrapper, Sender<ProcReply>)>,
}

impl ReadTask {
    /// Creates new instance of [`ReadTask`]
    pub fn new(
        socket: Arc<TcpStream>,
        client_addr: SocketAddr,
        mount_sender: Sender<MountCommand>,
        result_sender: Sender<ProcReply>,
        allocator: Arc<Impl>,
        pool_sender: Sender<(NfsArgWrapper, Sender<ProcReply>)>,
    ) -> Self {
        Self { socket, client_addr, mount_sender, result_sender, allocator, pool_sender }
    }

    /// Spawns a [`ReadTask`]  that reads commands from a socket.
    ///
    /// # Panics
    ///
    /// If called outside of tokio-uring runtime context.
    pub fn spawn(self) {
        tokio_uring::spawn(async move { self.run().await });
    }

    async fn run(self) -> io::Result<()> {
        let mut parser = RpcParser::new(UringReadStream::new(self.socket), self.allocator);

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

                    if let Err(err) = self.result_sender.send(result).await {
                        return send_broken_pipe(&self.result_sender, header.xid, err);
                    }
                }

                Ok(ArgWrapper { proc: ProcArguments::Nfs3(proc), header }) => {
                    let xid = header.xid;
                    debug!(client=%self.client_addr, xid, program="NFS", proc="NON_NULL", "rpc dispatch");
                    let command = NfsArgWrapper { header, proc };

                    if let Err(err) =
                        self.pool_sender.send((command, self.result_sender.clone())).await
                    {
                        return send_broken_pipe(&self.result_sender, xid, err);
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
                        return send_broken_pipe(&self.result_sender, xid, err);
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
                        return send_broken_pipe(&self.result_sender, xid, err);
                    }
                }

                Err(ErrorWrapper { xid: Some(xid), error }) => {
                    error!(client=%self.client_addr, xid, error=?error, "rpc parse error");
                    let result = ProcReply { xid, proc_result: Err(error) };
                    if let Err(err) = self.result_sender.send(result).await {
                        return send_broken_pipe(&self.result_sender, xid, err);
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

fn send_broken_pipe(
    sender: &Sender<ProcReply>,
    xid: u32,
    err: impl std::fmt::Display,
) -> io::Result<()> {
    sender
        .try_send(ProcReply {
            xid,
            proc_result: Err(Error::IO(io::Error::new(io::ErrorKind::BrokenPipe, err.to_string()))),
        })
        .map_err(|e| io::Error::new(io::ErrorKind::BrokenPipe, e))
}
