use std::num::NonZeroUsize;
use std::sync::Arc;

use tokio::net::TcpListener;

use nfs_mamont::{handle_forever, Impl, ServerContext};

use mock_vfs::config::MockVfsConfig;
use mock_vfs::mock::{MockMount, MockVfs};

#[cfg(debug_assertions)]
use nfs_mamont::init_tracing;

fn parse_args() -> String {
    let mut args = std::env::args().skip(1);
    let mut bind = "0.0.0.0:2049".to_string();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--bind" | "-b" => {
                bind = args.next().expect("--bind requires an address argument");
            }
            "--help" | "-h" => {
                eprintln!("Usage: mock-nfs-server [--bind <addr>]");
                eprintln!("  --bind | -b <addr>  Address to bind to (default: 0.0.0.0:2049)");
                std::process::exit(0);
            }
            _ => {
                eprintln!("Unknown argument: {arg}");
                eprintln!("Usage: mock-nfs-server [--bind <addr>]");
                std::process::exit(1);
            }
        }
    }
    bind
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    #[cfg(debug_assertions)]
    init_tracing();

    let bind_addr = parse_args();

    let config = MockVfsConfig::default();
    let backend = Arc::new(MockVfs::new(config));
    let buf_size = NonZeroUsize::new(1048576).unwrap();
    let buf_count = NonZeroUsize::new(64).unwrap();
    let read_alloc = Arc::new(Impl::new(buf_size, buf_count));
    let write_alloc = Arc::new(Impl::new(buf_size, buf_count));
    let pool_size = NonZeroUsize::new(4).unwrap();
    let context = ServerContext::new(backend, read_alloc, write_alloc, pool_size);
    let mount_service = Arc::new(MockMount);

    let listener = TcpListener::bind(&bind_addr).await?;
    let actual_addr = listener.local_addr()?;
    eprintln!("mock-nfs-server listening on {actual_addr}");

    handle_forever(listener, context, mount_service).await
}
