use std::sync::Arc;

use clap::Parser;
use tokio::net::TcpListener;
use tracing::info;

use nfs_mamont::vfs::file::Path as VfsPath;
use nfs_mamont::{handle_forever, service, Impl, ServerContext};

#[cfg(debug_assertions)]
use nfs_mamont::init_tracing;

pub mod args;
pub mod config;
pub mod fs;
pub mod fs_map;

#[cfg(test)]
mod tests;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    #[cfg(debug_assertions)]
    init_tracing();

    let args = args::Args::parse();

    let config = config::load_config(&args.config_path)?;

    let context = ServerContext::<_, _, fs::MirrorFS, _, _>::new(
        Arc::new(Impl::new(config.allocator.read_buffer_size, config.allocator.read_buffer_count)),
        Arc::new(Impl::new(
            config.allocator.write_buffer_size,
            config.allocator.write_buffer_count,
        )),
        config.vfs_pool_size,
    );

    // Keep a registry handle: the context itself is moved into the server task, while
    // backends are attached (and may be detached) after the server has started.
    let backends = context.backends();

    info!(bind = %args.addr, "mirrorfs startup");

    let listener = TcpListener::bind(&args.addr).await?;

    let mount_service = Arc::new(service::mount::MountService::with_exports(vec![]));
    let nlm_service = Arc::new(service::nlm::NlmService::new());

    let server_handle =
        tokio::spawn(handle_forever(listener, context, mount_service.clone(), nlm_service));

    let fs = Arc::new(fs::MirrorFS::new(config.export_root.clone()));
    let backend_id = backends
        .add(fs.clone())
        .ok_or_else(|| std::io::Error::other("no free backend slot left"))?;
    fs.set_backend_id(backend_id).await;

    info!(
        export_root = %config.export_root.display(),
        backend = backend_id,
        "mirrorfs backend attached"
    );

    for export in &config.exports {
        let root_handle = fs.handle_for_path(&export.local_path).await.map_err(|error| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "failed to resolve export handle for {}: {error:?}",
                    export.local_path.display()
                ),
            )
        })?;

        let directory = VfsPath::new(export.mount_path.clone())?;
        mount_service.add_export(directory, root_handle).await;
        info!(export = %export.mount_path, "export added");
    }

    server_handle.await.map_err(|_| std::io::Error::other("server task failed"))?
}
