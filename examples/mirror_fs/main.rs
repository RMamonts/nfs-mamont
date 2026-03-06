use std::path::PathBuf;
pub mod fs;
pub mod fs_map;

/// Main entry point for the mirror file system example
///
/// This function initializes the tracing subscriber, reads the directory path
/// from command line arguments, creates a MirrorFS instance, and starts
/// an NFS server on the specified port.
#[tokio::main]
async fn main() {
    let path = std::env::args().nth(1).expect("must supply directory to mirror");
    let path = PathBuf::from(path);

    let fs = fs::MirrorFS::new(path);
    let _ = fs.root_handle().await;
}
