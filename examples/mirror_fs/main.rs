use std::io::ErrorKind;
use std::sync::Arc;

use clap::Parser;
use tokio::net::TcpListener;
use tracing::info;

use nfs_mamont::{handle_forever_with_exports, MountExport, ServerContext};

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
    let args = args::Args::parse();

    let config = config::load_config(&args.config_path)?;
    let fs = Arc::new(fs::MirrorFS::new(config.export_root.clone()));
    let context = ServerContext::new(
        fs.clone(),
        config.allocator.read_buffer_size,
        config.allocator.read_buffer_count,
        config.allocator.write_buffer_size,
        config.allocator.write_buffer_count,
        config.vfs_pool_size,
    );

    #[cfg(debug_assertions)]
    init_tracing();

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
        exports.push(MountExport::from_directory_path(export.mount_path.clone(), root_handle)?);
    }

    handle_forever_with_exports(listener, context, exports).await
}
