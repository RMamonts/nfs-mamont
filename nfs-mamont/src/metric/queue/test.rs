use std::sync::Arc;

use metrics_util::debugging::DebugValue;
use metrics_util::CompositeKey;

use crate::metric::nfs::consts::QUEUE_DEPTH;
use crate::metric::test_helpers::{gauge_value, nfs_metrics, snapshot};
use crate::metric::{unbounded, Queue};

fn depth(snapshot: &[(CompositeKey, DebugValue)], queue: &str) -> f64 {
    gauge_value(snapshot, QUEUE_DEPTH, &[("queue", queue)])
}

#[tokio::test]
async fn queue_depth_tracks_each_queue() {
    let (metrics, snapshotter, _) = nfs_metrics();
    let metrics = Arc::new(metrics);
    let (vfs_tx, vfs_rx) = unbounded(Arc::clone(&metrics), Queue::Vfs);
    let (reply_tx, _reply_rx) = unbounded(metrics, Queue::Reply);

    for i in 0..3 {
        vfs_tx.send(i).await.unwrap();
    }
    vfs_rx.recv().await.unwrap();
    reply_tx.send(()).await.unwrap();

    let snapshot = snapshot(&snapshotter);

    assert_eq!(depth(&snapshot, "vfs"), 2.0);
    assert_eq!(depth(&snapshot, "reply"), 1.0);
}

#[tokio::test]
async fn queue_depth_is_restored_without_receiver() {
    let (metrics, snapshotter, _) = nfs_metrics();
    let (tx, rx) = unbounded(Arc::new(metrics), Queue::Vfs);
    let rx_clone = rx.clone();

    tx.send(()).await.unwrap();
    drop(rx);
    assert_eq!(depth(&snapshot(&snapshotter), "vfs"), 1.0);

    // taking snapshot resets the gauge to 0
    drop(rx_clone); // last rx dropped, gauge = -1
    assert!(tx.send(()).await.is_err()); // failed send doesn't count
    assert_eq!(depth(&snapshot(&snapshotter), "vfs"), -1.0);
}
