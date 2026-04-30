use std::sync::Arc;

use clap::Parser;
use tokio::net::TcpListener;
use tracing::info;

use nfs_mamont::{handle_forever_with_exports, MountExport, ServerContext};

#[cfg(debug_assertions)]
use nfs_mamont::init_tracing;

pub mod args_parser;
pub mod fs;
pub mod fs_map;

#[cfg(test)]
mod tests;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let args = args_parser::Args::parse();

    let export_root = std::fs::canonicalize(&args.path)
        .unwrap_or_else(|error| panic!("failed to resolve export root {}: {error}", &args.path));
    let metadata = std::fs::metadata(&export_root).unwrap_or_else(|error| {
        panic!("failed to stat export root {}: {error}", export_root.display())
    });
    assert!(metadata.is_dir(), "export root {} must be a directory", export_root.display());

    let fs = Arc::new(fs::MirrorFS::new(export_root.clone()));
    let root_handle = fs.root_handle().await;
    let context = ServerContext::new(
        fs.clone(),
        args.read_buffer_size,
        args.read_buffer_count,
        args.write_buffer_size,
        args.write_buffer_count,
        args.vfs_pool_size,
    );

    #[cfg(debug_assertions)]
    init_tracing();

    info!(export_root = %export_root.display(), bind = %args.bind, "mirrorfs startup");

    let listener = TcpListener::bind(&args.bind).await?;
    let export = MountExport::from_directory_path(export_root.to_string_lossy(), root_handle)?;

    handle_forever_with_exports(listener, context, vec![export]).await
}
