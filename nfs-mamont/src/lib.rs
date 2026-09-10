//! NFS Mamont - A Network File System (NFS) server implementation in Rust.

mod allocator;
pub mod consts;
mod context;

mod macros;
pub mod mount;
mod nlm;
mod parser;
mod rpc;
mod rpcbind;
mod serializer;
pub mod service;
mod task;
pub mod vfs;

use std::sync::Arc;

use tokio::net::TcpListener;
use tokio::signal;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

use crate::nlm::Nlm;
use crate::task::global::mount::MountTask;
use crate::task::global::nlm::NlmTask;
use crate::vfs::Vfs;
use crate::{mount::Mount, task::connection};

pub use allocator::{Allocator, Buffer, Impl, Slice, UnownedBuffer};
pub use context::ServerContext;

pub_use_if_feature!(
    "arbitrary",
    allocator::mock::alloc::MockAllocator,
    allocator::mock::buffer::MockBuffers,
    allocator::mock::buffer::MAX_BLOCK_AMOUNT,
    allocator::mock::buffer::TEST_SIZE,
    parser::parser_struct::RpcParser,
    parser::parser_struct::DEFAULT_SIZE,
    parser::parser_struct::RMS_HEADER_SIZE,
    parser::MountArguments,
    parser::NfsArguments,
    parser::NlmArguments,
    parser::ProcArguments,
    parser::primitive::u32_as_usize,
    parser::ArgWrapper,
    parser::ErrorWrapper,
    parser::nfsv3,
    parser::mount as parser_mount,
    parser::nlm as parser_nlm,
    consts::nfsv3::NFS_PROGRAM,
    consts::nfsv3::NFS_VERSION,
    consts::nfsv3::NULL,
    consts::nfsv3::GETATTR,
    consts::nfsv3::SETATTR,
    consts::nfsv3::LOOKUP,
    consts::nfsv3::ACCESS,
    consts::nfsv3::READLINK,
    consts::nfsv3::READ,
    consts::nfsv3::WRITE,
    consts::nfsv3::CREATE,
    consts::nfsv3::MKDIR,
    consts::nfsv3::SYMLINK,
    consts::nfsv3::MKNOD,
    consts::nfsv3::REMOVE,
    consts::nfsv3::RMDIR,
    consts::nfsv3::RENAME,
    consts::nfsv3::LINK,
    consts::nfsv3::READDIR,
    consts::nfsv3::READDIRPLUS,
    consts::nfsv3::FSSTAT,
    consts::nfsv3::FSINFO,
    consts::nfsv3::PATHCONF,
    consts::nfsv3::COMMIT,
    consts::mount::MOUNT_PROGRAM,
    consts::mount::MOUNT_VERSION,
    consts::mount::MOUNT_NULL,
    consts::mount::MOUNT_MNT,
    consts::mount::MOUNT_DUMP,
    consts::mount::MOUNT_UMNT,
    consts::mount::MOUNT_UMNTALL,
    consts::mount::MOUNT_EXPORT,
    serializer::client::arguments,
    serializer::server::serialize_struct::Serializer,
    rpc::AuthFlavor,
    rpc::Error,
    rpc::OpaqueAuth,
    rpc::RpcBody,
    rpc::RPC_VERSION,
    task::ProcReply,
);

/// Initializes tracing logs.
///
/// In debug builds logs are enabled by default. In release builds this is a no-op.
pub fn init_tracing() {
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("nfs_mamont=debug"));

    let _ = tracing_subscriber::fmt().with_env_filter(env_filter).try_init();
}

/// Starts the NFS server and processes client connections with explicit MOUNT exports.
pub async fn handle_forever<A, B, M, N, V>(
    listener: TcpListener,
    context: ServerContext<A, V, B>,
    mount_service: Arc<M>,
    nlm_service: Arc<N>,
) -> std::io::Result<()>
where
    A: Allocator<Buffer = B> + Send + Sync + 'static,
    B: Buffer + 'static,
    M: Mount + Send + Sync + 'static,
    N: Nlm + Send + Sync + 'static,
    V: Vfs<B> + Send + Sync + 'static,
{
    let (mount_task, mount_sender) = MountTask::new(mount_service);
    mount_task.spawn();

    let (nlm_task, nlm_sender) = NlmTask::new(nlm_service);
    nlm_task.spawn();

    // Publish the NFS/MOUNT/NLM services to the local rpcbind
    let port = listener.local_addr()?.port();
    match rpcbind::register(port).await {
        Ok(()) => info!(port, "registered NFS/MOUNT/NLM services with rpcbind"),
        Err(err) => warn!(
            port,
            error = %err,
            "failed to register with rpcbind; clients must pass port=/mountport= options"
        ),
    }

    // TODO: will be replaced with other solution in future
    let mut shutdown = Box::pin(signal::ctrl_c());

    let accept_result = loop {
        tokio::select! {
            accepted = listener.accept() => {
                match accepted {
                    Ok((socket, _)) => {
                        connection::new(socket, mount_sender.clone(), nlm_sender.clone(), &context).await;
                    }
                    Err(err) => break Err(err),
                }
            }
            result = &mut shutdown => {
                if matches!(result, Ok(())) {
                    break Ok(());
                }
            }
        }
    };

    let _ = rpcbind::unregister(port).await;

    accept_result
}
