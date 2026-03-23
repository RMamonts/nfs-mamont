use std::net::SocketAddr;

use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tracing::debug;

use crate::mount::dump::Dump;
use crate::mount::export::Export;
use crate::mount::mnt::Mnt;
use crate::mount::umnt::Umnt;
use crate::mount::umntall::Umntall;
use crate::mount::MountRes;
use crate::parser::{MountArgWrapper, MountArguments};
use crate::service::mount::{ExportEntryWrapper, MountService};
use crate::task::{ProcReply, ProcResult};

/// Command sent to [`MountTask`] from connection read tasks.
pub struct MountCommand {
    /// Channel used to pass the result to write task.
    pub result_tx: UnboundedSender<ProcReply>,
    /// Client socket address from connection task.
    pub client_addr: SocketAddr,
    /// Placeholder for mount procedure args.
    pub args: MountArgWrapper,
}

pub struct MountTask {
    #[allow(dead_code)]
    mount_service: MountService,
    // channel for commands from client connection tasks
    receiver: UnboundedReceiver<MountCommand>,
}

impl MountTask {
    /// Creates new instance of [`MountTask`]
    pub fn new(exports: Vec<ExportEntryWrapper>) -> (Self, UnboundedSender<MountCommand>) {
        let (sender, receiver) = mpsc::unbounded_channel::<MountCommand>();

        let task = Self { mount_service: MountService::with_exports(exports), receiver };

        (task, sender)
    }

    /// Spawns a [`MountTask`]  that processes mount commands received from
    /// `ReadTask` and returns results to
    /// `WriteTask`.
    ///
    /// # Panics
    ///
    /// If called outside of tokio runtime context.
    pub fn spawn(self) {
        tokio::spawn(async move { self.run().await });
    }

    async fn run(self) {
        let mut mount_service = self.mount_service;
        let mut receiver = self.receiver;

        while let Some(command) = receiver.recv().await {
            let MountCommand { result_tx, client_addr, args } = command;
            let MountArgWrapper { header, proc } = args;
            debug!(client=%client_addr, xid=header.xid, "mount task: command received");

            let mount_result = match *proc {
                MountArguments::Null => MountRes::Null,
                MountArguments::Mount(args) => {
                    debug!(xid=header.xid, dirpath=%args.dirpath.as_path().to_string_lossy(), "mount task: proc=MNT");
                    let res = mount_service.mnt(args, client_addr, header.cred).await;
                    match &res {
                        Ok(_) => {
                            debug!(xid = header.xid, "mount task: proc=MNT result=OK");
                        }
                        Err(status) => {
                            debug!(xid=header.xid, status=?status, "mount task: proc=MNT result=ERR");
                        }
                    }
                    MountRes::Mount(res)
                }
                MountArguments::Unmount(args) => {
                    debug!(xid=header.xid, dirpath=%args.dirpath.as_path().to_string_lossy(), "mount task: proc=UMNT");
                    mount_service.umnt(args, client_addr).await;
                    MountRes::Unmount
                }
                MountArguments::Export => {
                    debug!(xid = header.xid, "mount task: proc=EXPORT");
                    let res = mount_service.export().await;
                    debug!(
                        xid = header.xid,
                        entries = res.exports.len(),
                        "mount task: proc=EXPORT"
                    );
                    MountRes::Export(res)
                }
                MountArguments::Dump => {
                    debug!(xid = header.xid, "mount task: proc=DUMP");
                    let res = mount_service.dump().await;
                    debug!(
                        xid = header.xid,
                        entries = res.mount_list.len(),
                        "mount task: proc=DUMP"
                    );
                    MountRes::Dump(res)
                }
                MountArguments::UnmountAll => {
                    debug!(xid = header.xid, "mount task: proc=UMNTALL");
                    mount_service.umntall(client_addr).await;
                    MountRes::UnmountAll
                }
            };

            // TODO:
            // - some logs when occurred error
            // - or retry with fail
            // * but don't stop task
            let _ = result_tx.send(ProcReply {
                xid: header.xid,
                proc_result: Ok(ProcResult::Mount(Box::new(mount_result))),
            });
            debug!(xid = header.xid, "mount task: reply queued");
        }
    }
}
