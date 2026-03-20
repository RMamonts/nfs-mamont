use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::Arc;

use nfs_mamont::mount::ExportEntry;
use nfs_mamont::service::mount::ExportEntryWrapper;
use nfs_mamont::vfs::file;
use nfs_mamont::{handle_forever_with_exports, init_tracing, ServerContext};
use tokio::net::TcpListener;
use tracing::info;

pub mod fs;
pub mod fs_map;

#[cfg(test)]
mod tests;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    init_tracing();

    let path = std::env::args().nth(1).expect("must supply directory to mirror");
    let path = PathBuf::from(path);
    let bind = std::env::args().nth(2).unwrap_or_else(|| "0.0.0.0:2049".to_string());
    let allocator_buffer_size = std::env::var("MIRRORFS_ALLOC_BUFFER_SIZE")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .and_then(NonZeroUsize::new)
        .unwrap_or_else(|| NonZeroUsize::new(1024 * 1024).unwrap());
    let allocator_buffer_count = std::env::var("MIRRORFS_ALLOC_BUFFER_COUNT")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .and_then(NonZeroUsize::new)
        .unwrap_or_else(|| NonZeroUsize::new(64).unwrap());
    let export_root = std::fs::canonicalize(&path).unwrap_or_else(|error| {
        panic!("failed to resolve export root {}: {error}", path.display())
    });
    let metadata = std::fs::metadata(&export_root).unwrap_or_else(|error| {
        panic!("failed to stat export root {}: {error}", export_root.display())
    });
    assert!(metadata.is_dir(), "export root {} must be a directory", export_root.display());

    let fs = Arc::new(fs::MirrorFS::new(export_root.clone()));
    let root_handle = fs.root_handle().await;
    let context = ServerContext::new(fs.clone(), allocator_buffer_size, allocator_buffer_count);

    info!(
        export_root = %export_root.display(),
        bind = %bind,
        allocator_buffer_size = allocator_buffer_size.get(),
        allocator_buffer_count = allocator_buffer_count.get(),
        "mirrorfs startup",
    );

    let listener = TcpListener::bind(&bind).await?;
    let export = ExportEntryWrapper {
        export: ExportEntry {
            directory: file::Path::new(export_root.to_string_lossy().into_owned()).unwrap(),
            names: Vec::new(),
        },
        root_handle,
    };

    handle_forever_with_exports(listener, context, vec![export]).await
}
