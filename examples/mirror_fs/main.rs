use std::sync::Arc;

use tokio::net::TcpListener;
use tracing::info;

use nfs_mamont::{handle_forever_with_exports, MountExport, ServerContext};

#[cfg(debug_assertions)]
use nfs_mamont::init_tracing;

mod config;
pub mod fs;
pub mod fs_map;

#[cfg(test)]
mod tests;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let config = config::parse_runtime_config(std::env::args_os())?;

    #[cfg(debug_assertions)]
    init_tracing();

    info!(bind = %config.bind, exports = config.exports.len(), "mirrorfs startup");

    let fs = Arc::new(fs::MirrorFS::new_many(
        config.exports.iter().map(|export| export.local_path.clone()).collect(),
    ));

    let context = ServerContext::new(
        fs.clone(),
        config.allocator.read_buffer_size,
        config.allocator.read_buffer_count,
        config.allocator.write_buffer_size,
        config.allocator.write_buffer_count,
        config.vfs_pool_size,
    );

    let mut exports = Vec::with_capacity(config.exports.len());
    for (export_id, configured_export) in config.exports.iter().enumerate() {
        info!(
            export_root = %configured_export.local_path.display(),
            mount_path = %configured_export.mount_path,
            "configured mirror export"
        );
        let root_handle = fs.root_handle_for_export(export_id).await;
        let export =
            MountExport::from_directory_path(configured_export.mount_path.clone(), root_handle)
                .map_err(|error| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!(
                            "failed to register export {}: {error}",
                            configured_export.mount_path
                        ),
                    )
                })?;
        exports.push(export);
    }

    let listener = TcpListener::bind(config.bind).await?;
    handle_forever_with_exports(listener, context, exports).await
}
