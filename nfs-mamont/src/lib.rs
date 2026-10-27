//! NFS Mamont - A Network File System (NFS) server implementation in Rust.

mod allocator;
mod backend;
pub mod consts;
mod context;
pub mod mount;
#[allow(dead_code)]
mod nlm;
mod parser;
mod rpc;
mod rpcbind;
mod serializer;
pub mod service;
mod task;
mod telemetry;
pub mod vfs;

#[cfg(feature = "metrics")]
use std::net::SocketAddr;
use std::sync::Arc;

use tokio::net::TcpListener;
use tokio::signal;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

use crate::nlm::Nlm;
use crate::task::global::mount::MountTask;
use crate::task::global::nlm::nlm_task::NlmTask;
use crate::vfs::Vfs;
use crate::{mount::Mount, task::connection};

pub use allocator::{Allocator, Buffer, Impl, Slice, UnownedBuffer};
pub use backend::BackendRegistry;
pub use context::ServerContext;
pub use rpc::auth;
pub use vfs::file::BackendId;

/// Installs a Prometheus scrape endpoint for metrics emitted by this process.
///
/// Requires the `metrics` feature. Call this before creating allocators and server
/// contexts. The application chooses the address to control endpoint reachability.
#[cfg(feature = "metrics")]
pub fn install_prometheus_exporter(
    address: SocketAddr,
) -> Result<(), metrics_exporter_prometheus::BuildError> {
    metrics_exporter_prometheus::PrometheusBuilder::new()
        .with_http_listener(address)
        .set_buckets_for_metric(
            metrics_exporter_prometheus::Matcher::Suffix("_seconds".into()),
            telemetry::DURATION_BUCKETS,
        )?
        .install()
}

/// Publishes the current runtime/task samples and VFS queue depths immediately.
///
/// Call inside each monitored runtime after the workload has drained, then keep
/// the runtime and exporter alive until Prometheus has scraped the final sample.
/// This does not write a durable archive or force Prometheus to scrape.
#[cfg(feature = "metrics")]
pub fn flush_metrics() {
    telemetry::flush();
}

/// Initializes tracing logs.
///
/// In debug builds logs are enabled by default. In release builds this is a no-op.
pub fn init_tracing() {
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("nfs_mamont=debug"));

    let _ = tracing_subscriber::fmt().with_env_filter(env_filter).try_init();
}

/// Starts the NFS server and processes client connections with explicit MOUNT exports.
///
/// With the `metrics` feature, shutdown publishes the final metrics and keeps the
/// runtime available for another 11 seconds before returning, allowing Prometheus
/// to scrape them with the bundled five-second scrape interval. This grace period
/// does not wait for independently spawned connection tasks to finish.
pub async fn handle_forever<A, B, M, N, V>(
    listener: TcpListener,
    context: ServerContext<A, V, B>,
    mount_service: Arc<M>,
    nlm_service: Arc<N>,
) -> std::io::Result<()>
where
    A: Allocator<Buffer = B> + Send + Sync + 'static,
    B: Buffer + 'static,
    M: Mount + Send + Sync + 'static,
    N: Nlm + Send + Sync + 'static,
    V: Vfs<B> + Send + Sync + 'static,
{
    telemetry::start();

    let (mount_task, mount_sender) = MountTask::new(mount_service);
    mount_task.spawn();

    let (nlm_task, nlm_sender) = NlmTask::new(nlm_service);
    nlm_task.spawn();

    // Publish the NFS/MOUNT/NLM services to the local rpcbind
    let port = listener.local_addr()?.port();
    match rpcbind::register(port).await {
        Ok(()) => info!(port, "registered NFS/MOUNT/NLM services with rpcbind"),
        Err(err) => warn!(
            port,
            error = %err,
            "failed to register with rpcbind; clients must pass port=/mountport= options"
        ),
    }

    // TODO: will be replaced with other solution in future
    let mut shutdown = Box::pin(signal::ctrl_c());

    let accept_result = loop {
        tokio::select! {
            accepted = listener.accept() => {
                match accepted {
                    Ok((socket, _)) => {
                        connection::new(socket, mount_sender.clone(), nlm_sender.clone(), &context);
                    }
                    Err(err) => break Err(err),
                }
            }
            result = &mut shutdown => {
                if matches!(result, Ok(())) {
                    break Ok(());
                }
            }
        }
    };

    let _ = rpcbind::unregister(port).await;
    #[cfg(feature = "metrics")]
    {
        flush_metrics();
        info!(grace_seconds = 11, "keeping metrics available for final Prometheus scrapes");
        tokio::time::sleep(std::time::Duration::from_secs(11)).await;
    }

    accept_result
}
