use std::io;
use std::net::SocketAddr;
use std::sync::Arc;

use tokio::net::tcp::OwnedReadHalf;
use tokio::sync::mpsc::UnboundedSender;
use tracing::{debug, error};

use async_channel::Sender;

use crate::allocator::Impl;
use crate::mount::MountRes;
use crate::parser::parser_struct::RpcParser;
use crate::parser::{
    ArgWrapper, ErrorWrapper, MountArgWrapper, MountArguments, NfsArgWrapper, NfsArguments,
    ProcArguments,
};
use crate::rpc::Error;
use crate::task::global::mount::MountCommand;
use crate::task::{ProcReply, ProcResult};
use crate::vfs::NfsRes;

/// Reads RPC commands from a network connection, parses them,
/// and forwards to [`crate::task::connection::vfs::VfsTask`] or global tasks.
pub struct ReadTask {
    readhalf: OwnedReadHalf,
    client_addr: SocketAddr,
    // to send messages into mount task
    mount_sender: UnboundedSender<MountCommand>,
    // to pass into mount task as part of message,
    // so mount task can send result back to write task
    // and
    // to bypass vfs with null procedure
    result_sender: UnboundedSender<ProcReply>,
    allocator: Arc<Impl>,
    cmd_sender: Sender<(NfsArgWrapper, UnboundedSender<ProcReply>)>,
}

impl ReadTask {
    /// Creates new instance of [`ReadTask`]
    pub fn new(
        readhalf: OwnedReadHalf,
        client_addr: SocketAddr,
        mount_sender: UnboundedSender<MountCommand>,
        result_sender: UnboundedSender<ProcReply>,
        allocator: Arc<Impl>,
        cmd_sender: Sender<(NfsArgWrapper, UnboundedSender<ProcReply>)>,
    ) -> Self {
        Self { readhalf, client_addr, mount_sender, result_sender, allocator, cmd_sender }
    }

    /// Spawns a [`ReadTask`]  that reads commands from a socket.
    ///
    /// # Panics
    ///
    /// If called outside of tokio runtime context.
    pub fn spawn(self) {
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

                    if let Err(err) = self.result_sender.send(result) {
                        // return send_broken_pipe(&self.result_sender, header.xid, err);
                    }
                }

                Ok(ArgWrapper { proc: ProcArguments::Nfs3(proc), header }) => {
                    let xid = header.xid;
                    debug!(client=%self.client_addr, xid, program="NFS", proc="NON_NULL", "rpc dispatch");
                    let command = NfsArgWrapper { header, proc };

                    if let Err(err) =
                        self.cmd_sender.send((command, self.result_sender.clone())).await
                    {
                        // return send_broken_pipe(&self.result_sender, xid, err).await?;
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

                    if let Err(err) = self.result_sender.send(result) {
                        // return send_broken_pipe(&self.result_sender, xid, err);
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
                    if let Err(err) = self.mount_sender.send(command) {
                        // return send_broken_pipe(&self.result_sender, xid, err);
                    }
                }

                Err(ErrorWrapper { xid: Some(xid), error }) => {
                    error!(client=%self.client_addr, xid, error=?error, "rpc parse error");
                    let result = ProcReply { xid, proc_result: Err(error) };
                    if let Err(err) = self.result_sender.send(result) {
                        // return send_broken_pipe(&self.result_sender, xid, err);
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

// async fn send_broken_pipe(
//     sender: &tokio::sync::mpsc::Sender<ProcReply>,
//     xid: u32,
//     err: impl std::fmt::Display,
// ) -> io::Result<()> {
//     let a = sender
//         .send(ProcReply {
//             xid,
//             proc_result: Err(Error::IO(io::Error::new(io::ErrorKind::BrokenPipe, err.to_string()))),
//         }).await.map_err(|e| io::Error::new(io::ErrorKind::BrokenPipe, e))
// }
