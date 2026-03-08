use std::path::PathBuf;
use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

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
    // let _ = tracing_subscriber::fmt()
    //     .with_max_level(tracing::Level::DEBUG)
    //     .with_writer(std::io::stderr)
    //     .with_env_filter(
    //         EnvFilter::try_from_default_env()
    //             .unwrap_or_else(|_| EnvFilter::new("info,nfs_mamont=debug")),
    //     )
    //     .try_init();

    let path = std::env::args().nth(1).expect("must supply directory to mirror");
    let path = PathBuf::from(path);
    let listen = std::env::args().nth(2).unwrap_or_else(|| "0.0.0.0:2049".to_string());
    let metrics_listen = std::env::args().nth(3);
    let export_root = std::fs::canonicalize(&path).unwrap_or_else(|error| {
        panic!("failed to resolve export root {}: {error}", path.display())
    });
    let metadata = std::fs::metadata(&export_root).unwrap_or_else(|error| {
        panic!("failed to stat export root {}: {error}", export_root.display())
    });
    assert!(metadata.is_dir(), "export root {} must be a directory", export_root.display());

    let fs = Arc::new(fs::MirrorFS::new(export_root.clone()));
    let context = ServerContext::with_backend(fs);
    context
        .add_export(ServerExport::new(
            file::Path::new(export_root.display().to_string()).expect("export path must fit"),
            vec!["*".to_string()],
        ))
        .await;

    if let Some(metrics_listen) = metrics_listen {
        let metrics_context = context.clone();
        tokio::spawn(async move {
            if let Err(error) =
                serve_metrics_endpoint(metrics_listen.clone(), metrics_context).await
            {
                warn!(listen = %metrics_listen, error = %error, "metrics endpoint stopped");
            }
        });
    }

    let listener = TcpListener::bind(&listen).await.expect("failed to bind listener");
    info!(listen, export = %export_root.display(), "mirrorfs listening");
    nfs_mamont::handle_forever_with_context(listener, context).await.expect("server loop failed");
}

async fn serve_metrics_endpoint(listen: String, context: ServerContext) -> std::io::Result<()> {
    let listener = TcpListener::bind(&listen).await?;
    info!(listen, "metrics endpoint listening");

    loop {
        let (mut socket, peer) = listener.accept().await?;
        let metrics = context.metrics();
        tokio::spawn(async move {
            let mut buf = [0u8; 1024];
            let read = match socket.read(&mut buf).await {
                Ok(read) => read,
                Err(error) => {
                    warn!(peer = %peer, error = %error, "metrics endpoint read failed");
                    return;
                }
            };

            let request = std::str::from_utf8(&buf[..read]).unwrap_or_default();
            let response = if request.starts_with("GET /metrics ") {
                metrics_response(&metrics.encode_prometheus())
            } else {
                not_found_response()
            };

            if let Err(error) = socket.write_all(response.as_bytes()).await {
                warn!(peer = %peer, error = %error, "metrics endpoint write failed");
            }
        });
    }
}

fn metrics_response(body: &str) -> String {
    format!(
        "HTTP/1.1 200 OK\r\ncontent-type: text/plain; version=0.0.4\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
        body.len(),
        body,
    )
}

fn not_found_response() -> String {
    let body = "not found\n";
    format!(
        "HTTP/1.1 404 Not Found\r\ncontent-type: text/plain; charset=utf-8\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
        body.len(),
        body,
    )
}
