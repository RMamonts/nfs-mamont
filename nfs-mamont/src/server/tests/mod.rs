use std::net::SocketAddr;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;
use tokio::sync::Notify;
use tokio::time::timeout;

use crate::allocator::{Buffer, Impl};
use crate::consts::rpc::HEADER_MASK;
use crate::mount::{dump, export, mnt, umnt, umntall};
use crate::rpc::auth::Credential;
use crate::rpc::{AuthFlavor, RpcBody, RPC_VERSION};
use crate::vfs;

mod serve;
mod server;

/// Upper bound for every wait, so a broken shutdown fails a test instead of hanging it.
const WAIT: Duration = Duration::from_secs(5);

/// Filesystem that is never attached, so none of its procedures is ever called.
struct StubVfs;

macro_rules! unreachable_procedures {
    ($($module:ident::$procedure:ident::$method:ident),* $(,)?) => {
        $(
            impl vfs::$module::$procedure for StubVfs {
                async fn $method(
                    &self,
                    _: vfs::$module::Args,
                    _: &Credential,
                ) -> Result<vfs::$module::Success, vfs::$module::Fail> {
                    unreachable!("no backend is attached")
                }
            }
        )*
    };
}

unreachable_procedures!(
    get_attr::GetAttr::get_attr,
    set_attr::SetAttr::set_attr,
    lookup::Lookup::lookup,
    access::Access::access,
    read_link::ReadLink::read_link,
    create::Create::create,
    mk_dir::MkDir::mk_dir,
    symlink::Symlink::symlink,
    mk_node::MkNode::mk_node,
    remove::Remove::remove,
    rm_dir::RmDir::rm_dir,
    rename::Rename::rename,
    link::Link::link,
    read_dir::ReadDir::read_dir,
    read_dir_plus::ReadDirPlus::read_dir_plus,
    fs_stat::FsStat::fs_stat,
    fs_info::FsInfo::fs_info,
    path_conf::PathConf::path_conf,
    commit::Commit::commit,
);

impl<B: Buffer> vfs::read::Read<B> for StubVfs {
    async fn read(
        &self,
        _: vfs::read::Args,
        _: B,
        _: &Credential,
    ) -> Result<vfs::read::Success<B>, vfs::read::Fail> {
        unreachable!("no backend is attached")
    }
}

impl<B: Buffer> vfs::write::Write<B> for StubVfs {
    async fn write(
        &self,
        _: vfs::write::Args<B>,
        _: &Credential,
    ) -> Result<vfs::write::Success, vfs::write::Fail> {
        unreachable!("no backend is attached")
    }
}

/// MOUNT service whose `EXPORT` blocks until the test releases it.
#[derive(Default)]
struct GatedMount {
    /// Notified once `EXPORT` starts executing.
    entered: Notify,
    /// Lets the blocked `EXPORT` finish.
    released: Notify,
}

impl mnt::Mnt for GatedMount {
    async fn mnt(
        &self,
        _: mnt::Args,
        _: SocketAddr,
        _: &Credential,
    ) -> Result<mnt::Success, mnt::Fail> {
        Err(mnt::Fail::NotSupp)
    }
}

impl umnt::Umnt for GatedMount {
    async fn umnt(&self, _: umnt::Args, _: SocketAddr, _: &Credential) {}
}

impl umntall::Umntall for GatedMount {
    async fn umntall(&self, _: SocketAddr, _: &Credential) {}
}

impl export::Export for GatedMount {
    async fn export(&self, _: &Credential) -> export::Success {
        self.entered.notify_one();
        self.released.notified().await;

        export::Success { exports: Vec::new() }
    }
}

impl dump::Dump for GatedMount {
    async fn dump(&self, _: &Credential) -> dump::Success {
        dump::Success { mount_list: Vec::new() }
    }
}

/// Returns a small allocator, enough for requests without payload.
fn allocator() -> Arc<Impl> {
    Arc::new(Impl::new(NonZeroUsize::new(4096).unwrap(), NonZeroUsize::new(4).unwrap()))
}

/// Encodes a framed RPC call with `AUTH_NONE` credentials and no arguments.
fn call(xid: u32, program: u32, version: u32, procedure: u32) -> Vec<u8> {
    let words = [
        xid,
        RpcBody::Call as u32,
        RPC_VERSION,
        program,
        version,
        procedure,
        AuthFlavor::None as u32,
        0,
        AuthFlavor::None as u32,
        0,
    ];
    let marker = HEADER_MASK as u32 | (words.len() * 4) as u32;

    std::iter::once(marker).chain(words).flat_map(u32::to_be_bytes).collect()
}

/// Reads one reply record and returns its xid.
async fn reply_xid(stream: &mut TcpStream) -> u32 {
    let mut marker = [0; 4];
    stream.read_exact(&mut marker).await.unwrap();

    let mut body = vec![0; (u32::from_be_bytes(marker) & !(HEADER_MASK as u32)) as usize];
    stream.read_exact(&mut body).await.unwrap();

    u32::from_be_bytes(body[..4].try_into().unwrap())
}

/// Asserts that the server has closed `stream`.
async fn assert_closed(stream: &mut TcpStream) {
    let mut byte = [0; 1];
    let read = timeout(WAIT, stream.read(&mut byte)).await.expect("connection is still open");

    // A reset is fine as well: the server may close a connection it has not fully read.
    assert_eq!(read.unwrap_or(0), 0, "unexpected data after shutdown");
}
