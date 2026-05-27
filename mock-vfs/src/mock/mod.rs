use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};

use nfs_mamont::consts::nfsv3::NFS3_FHSIZE;
use nfs_mamont::mount::{dump, export, mnt, umnt, umntall};
use nfs_mamont::vfs::file::Handle;
use nfs_mamont::vfs::WccData;
use nfs_mamont::OpaqueAuth;

use crate::config::MockVfsConfig;

mod access_impl;
mod commit_impl;
mod create_impl;
mod fs_info_impl;
mod fs_stat_impl;
mod get_attr_impl;
mod link_impl;
mod lookup_impl;
mod mk_dir_impl;
mod mk_node_impl;
mod path_conf_impl;
mod read_dir_impl;
mod read_dir_plus_impl;
mod read_impl;
mod read_link_impl;
mod remove_impl;
mod rename_impl;
mod rm_dir_impl;
mod set_attr_impl;
mod symlink_impl;
mod write_impl;

pub const ROOT_HANDLE: Handle = {
    let mut h = [0u8; NFS3_FHSIZE];
    h[7] = 1;
    Handle(h)
};

fn next_handle(counter: &AtomicU64) -> Handle {
    let id = counter.fetch_add(1, Ordering::Relaxed);
    let mut bytes = [0u8; NFS3_FHSIZE];
    let be = id.to_be_bytes();
    let n = be.len().min(NFS3_FHSIZE);
    bytes[..n].copy_from_slice(&be[..n]);
    Handle(bytes)
}

pub fn file_attr(config: &MockVfsConfig) -> nfs_mamont::vfs::file::Attr {
    nfs_mamont::vfs::file::Attr {
        size: config.file_size,
        used: config.file_size,
        ..config.default_attr
    }
}

pub struct MockVfs {
    config: MockVfsConfig,
    next_handle: AtomicU64,
}

impl MockVfs {
    pub fn new(config: MockVfsConfig) -> Self {
        Self { config, next_handle: AtomicU64::new(2) }
    }

    pub fn file_attr(&self) -> nfs_mamont::vfs::file::Attr {
        file_attr(&self.config)
    }

    pub fn wcc_data(&self) -> WccData {
        let attr = self.file_attr();
        WccData {
            before: Some(nfs_mamont::vfs::file::WccAttr {
                size: attr.size,
                mtime: attr.mtime,
                ctime: attr.ctime,
            }),
            after: Some(attr),
        }
    }

    pub fn dir_wcc(&self) -> WccData {
        WccData {
            before: Some(nfs_mamont::vfs::file::WccAttr {
                size: 4096,
                mtime: self.config.dir_attr.mtime,
                ctime: self.config.dir_attr.ctime,
            }),
            after: Some(self.config.dir_attr.clone()),
        }
    }

    pub fn next_handle(&self) -> Handle {
        next_handle(&self.next_handle)
    }
}

pub struct MockMount;

impl mnt::Mnt for MockMount {
    async fn mnt(
        &self,
        _args: mnt::Args,
        _client_addr: SocketAddr,
        _cred: OpaqueAuth,
    ) -> Result<mnt::Success, mnt::Fail> {
        Err(mnt::Fail::Access)
    }
}

impl umnt::Umnt for MockMount {
    async fn umnt(&self, _args: umnt::Args, _client_addr: SocketAddr) {}
}

impl umntall::Umntall for MockMount {
    async fn umntall(&self, _client_addr: SocketAddr) {}
}

impl export::Export for MockMount {
    async fn export(&self) -> export::Success {
        export::Success { exports: vec![] }
    }
}

impl dump::Dump for MockMount {
    async fn dump(&self) -> dump::Success {
        dump::Success { mount_list: vec![] }
    }
}
