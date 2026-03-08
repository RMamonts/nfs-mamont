use std::path::PathBuf;
use std::sync::Arc;

use tokio::net::TcpListener;

use nfs_mamont::rpc::{ServerContext, ServerExport};
use nfs_mamont::vfs::file;

pub mod fs;
pub mod fs_map;

#[cfg(test)]
mod tests;

/// Main entry point for the mirror file system example
///
/// This function initializes the tracing subscriber, reads the directory path
/// from command line arguments, creates a MirrorFS instance, and starts
/// an NFS server on the specified port.
#[tokio::main]
async fn main() {
    let path = std::env::args().nth(1).expect("must supply directory to mirror");
    let path = PathBuf::from(path);
    let listen = std::env::args().nth(2).unwrap_or_else(|| "0.0.0.0:2049".to_string());
    let export_root = std::fs::canonicalize(&path).unwrap_or_else(|error| {
        panic!("failed to resolve export root {}: {error}", path.display())
    });
    let metadata = std::fs::metadata(&export_root).unwrap_or_else(|error| {
        panic!("failed to stat export root {}: {error}", export_root.display())
    });
    assert!(metadata.is_dir(), "export root {} must be a directory", export_root.display());

    let fs = Arc::new(fs::MirrorFS::new(export_root.clone()));
    let context = ServerContext::with_backend(fs);
    context.exports.write().expect("exports lock poisoned").push(ServerExport {
        directory: file::Path::new(export_root.display().to_string())
            .expect("export path must fit"),
        allowed_hosts: vec!["*".to_string()],
    });

    let listener = TcpListener::bind(&listen).await.expect("failed to bind listener");
    eprintln!("mirrorfs listening on {listen}, export {}", export_root.display());
    nfs_mamont::handle_forever_with_context(listener, context).await.expect("server loop failed");
}
