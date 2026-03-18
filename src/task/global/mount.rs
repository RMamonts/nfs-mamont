use std::net::SocketAddr;

use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

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
    /// [`crate::task::connection::read::ReadTask`] and returns results to
    /// [`crate::task::connection::write::WriteTask`].
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
            crate::debug_log!(
                "mount task: client={} xid={} command received",
                client_addr,
                header.xid
            );

            let mount_result = match *proc {
                MountArguments::Null => MountRes::Null,
                MountArguments::Mount(args) => {
                    crate::debug_log!(
                        "mount task: xid={} proc=MNT dirpath='{}'",
                        header.xid,
                        args.dirpath.as_path().to_string_lossy()
                    );
                    let res = mount_service.mnt(args, client_addr, header.cred).await;
                    match &res {
                        Ok(_) => {
                            crate::debug_log!("mount task: xid={} proc=MNT result=OK", header.xid)
                        }
                        Err(status) => {
                            crate::debug_log!(
                                "mount task: xid={} proc=MNT result=ERR {:?}",
                                header.xid,
                                status
                            )
                        }
                    }
                    MountRes::Mount(res)
                }
                MountArguments::Unmount(args) => {
                    crate::debug_log!(
                        "mount task: xid={} proc=UMNT dirpath='{}'",
                        header.xid,
                        args.dirpath.as_path().to_string_lossy()
                    );
                    mount_service.umnt(args, client_addr).await;
                    MountRes::Unmount
                }
                MountArguments::Export => {
                    crate::debug_log!("mount task: xid={} proc=EXPORT", header.xid);
                    let res = mount_service.export().await;
                    crate::debug_log!(
                        "mount task: xid={} proc=EXPORT entries={}",
                        header.xid,
                        res.exports.len()
                    );
                    MountRes::Export(res)
                }
                MountArguments::Dump => {
                    crate::debug_log!("mount task: xid={} proc=DUMP", header.xid);
                    let res = mount_service.dump().await;
                    crate::debug_log!(
                        "mount task: xid={} proc=DUMP entries={}",
                        header.xid,
                        res.mount_list.len()
                    );
                    MountRes::Dump(res)
                }
                MountArguments::UnmountAll => {
                    crate::debug_log!("mount task: xid={} proc=UMNTALL", header.xid);
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
            crate::debug_log!("mount task: xid={} reply queued", header.xid);
        }
    }
}
