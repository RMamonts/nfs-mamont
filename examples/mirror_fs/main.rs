use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::net::TcpListener;
use tracing::info;

use nfs_mamont::{handle_forever_with_exports, MountExport, ServerContext};

#[cfg(debug_assertions)]
use nfs_mamont::init_tracing;

pub mod fs;
pub mod fs_map;

#[cfg(test)]
mod tests;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let path = std::env::args().nth(1).expect("must supply directory to mirror");
    let path = PathBuf::from(path);
    let bind = std::env::args().nth(2).unwrap_or_else(|| "0.0.0.0:2049".to_string());
    let export_root = std::fs::canonicalize(&path).unwrap_or_else(|error| {
        panic!("failed to resolve export root {}: {error}", path.display())
    });
    let metadata = std::fs::metadata(&export_root).unwrap_or_else(|error| {
        panic!("failed to stat export root {}: {error}", export_root.display())
    });
    assert!(metadata.is_dir(), "export root {} must be a directory", export_root.display());

    let fs = Arc::new(fs::MirrorFS::new(export_root.clone()));
    let root_handle = fs.root_handle().await;
    let context = ServerContext::new(
        fs.clone(),
        NonZeroUsize::new(1024 * 1024).unwrap(),
        NonZeroUsize::new(1024).unwrap(),
    );

    #[cfg(debug_assertions)]
    init_tracing();

    info!(export_root = %export_root.display(), bind = %bind, "mirrorfs startup");

    let listener = TcpListener::bind(&bind).await?;
    let export = MountExport::from_directory_path(export_root.to_string_lossy(), root_handle)?;

    handle_forever_with_exports(listener, context, vec![export]).await
}
