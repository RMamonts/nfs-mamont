use std::net::SocketAddr;
use std::sync::Arc;

use async_channel;
use tracing::debug;

use crate::mount::{Mount, MountRes};
use crate::parser::{MountArgWrapper, MountArguments};
use crate::task::{ProcReply, ProcResult};

pub struct MountCommand {
    pub result_tx: async_channel::Sender<ProcReply>,
    pub client_addr: SocketAddr,
    pub args: MountArgWrapper,
}

pub struct MountTask<M>
where
    M: Mount + Send + Sync + 'static,
{
    mount_service: Arc<M>,
    receiver: async_channel::Receiver<MountCommand>,
}

impl<M> MountTask<M>
where
    M: Mount + Send + Sync + 'static,
{
    pub fn new(mount_service: Arc<M>) -> (Self, async_channel::Sender<MountCommand>) {
        let (sender, receiver) = async_channel::unbounded::<MountCommand>();

        let task = Self { mount_service, receiver };

        (task, sender)
    }

    pub fn spawn(self) {
        monoio::spawn(async move { self.run().await });
    }

    async fn run(self) {
        let mount_service = self.mount_service;
        let receiver = self.receiver;

        while let Ok(command) = receiver.recv().await {
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

            let _ = result_tx.send(ProcReply {
                xid: header.xid,
                proc_result: Ok(ProcResult::Mount(Box::new(mount_result))),
            }).await;
            debug!(xid = header.xid, "mount task: reply queued");
        }
    }
}
