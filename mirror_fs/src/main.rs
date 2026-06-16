use std::io::ErrorKind;
use std::sync::Arc;

use clap::Parser;
use tokio::net::TcpListener;
use tracing::info;

use nfs_mamont::mount::ExportEntry;
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
    let fs = Arc::new(fs::MirrorFS::new(config.export_root.clone()));

    let context = ServerContext::new(
        fs.clone(),
        Arc::new(Impl::new(config.allocator.read_buffer_size, config.allocator.read_buffer_count)),
        Arc::new(Impl::new(
            config.allocator.write_buffer_size,
            config.allocator.write_buffer_count,
        )),
        config.vfs_pool_size,
    );

    info!(export_root = %config.export_root.display(), bind = %args.addr, "mirrorfs startup");

    let listener = TcpListener::bind(&args.addr).await?;

    let mut exports = Vec::with_capacity(config.exports.len());
    for export in &config.exports {
        let root_handle = fs.handle_for_path(&export.local_path).await.map_err(|error| {
            std::io::Error::new(
                ErrorKind::InvalidInput,
                format!(
                    "failed to resolve export handle for {}: {error:?}",
                    export.local_path.display()
                ),
            )
        })?;

        exports.push(service::mount::ExportEntryWrapper {
            export: ExportEntry {
                directory: VfsPath::new(export.mount_path.clone())?,
                names: Vec::new(),
            },
            root_handle,
        });
    }

    let mount_service = Arc::new(service::mount::MountService::with_exports(exports));
    let nlm_service = Arc::new(service::nlm::NlmService::new());
    handle_forever(listener, context, mount_service, nlm_service).await
}
