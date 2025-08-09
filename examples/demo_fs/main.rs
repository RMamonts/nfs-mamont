use clap::Parser;
use clap_derive::Parser;

use nfs_mamont::tcp::{NFSTcp, NFSTcpListener};

/// Implements the core file system functionality
mod fs;
/// Defines the storage representation for file system entries
mod fs_contents;
/// Defines the structure for file system entry metadata and content
mod fs_entry;

/// Port number on which the NFS server will listen
const HOSTPORT: u32 = 11111;

#[derive(Parser, Debug)]
struct Args {
    #[arg(long)]
    multi_export: bool,
}

/// Demo NFS server implementation using the nfs-mamont library.
/// Shows how to create a simple in-memory file system that supports NFS operations.
#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_writer(std::io::stderr)
        .init();

    let args = Args::parse();

    println!("Starting NFS server on 0.0.0.0:{HOSTPORT}");

    let mut listener = NFSTcpListener::bind(&format!("0.0.0.0:{HOSTPORT}")).await.unwrap();
    if !args.multi_export {
        let fs = fs::DemoFS::default();
        listener.register_export(fs).await.unwrap();
    } else {
        let fs_one = fs::DemoFS::default();
        let fs_two = fs::DemoFS::default();
        listener.register_export_with_name(fs_one, "one").await.unwrap();
        listener.register_export_with_name(fs_two, "two").await.unwrap();
    }
    listener.handle_forever().await.unwrap();
}
