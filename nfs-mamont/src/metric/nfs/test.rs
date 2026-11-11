use metrics::with_local_recorder;
use metrics_util::debugging::DebugValue;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::consts::{
    CPU_SECONDS, DURATION_SECONDS, REQUESTS_TOTAL, STAGE_SECONDS, SYSCALL_SECONDS, VFS_IN_FLIGHT,
    WAIT_SECONDS,
};
use crate::consts::nfsv3::GETATTR;
use crate::metric::test_helpers::{gauge, nfs_metrics as metrics, samples, snapshot, value};
use crate::metric::{Stage, TaskKind, Timer};

#[test]
fn timer_is_16_bytes() {
    assert_eq!(std::mem::size_of::<Timer>(), 16);
}

#[tokio::test]
async fn unmeasured_timer_records_nothing() {
    let (metrics, snapshotter, _) = metrics();

    let mut timer = Timer::NONE;
    metrics.lap(&mut timer, Stage::Vfs);
    metrics.finish(timer, true);

    let snapshot = snapshot(&snapshotter);

    assert_eq!(value(&snapshot, REQUESTS_TOTAL, &[("status", "ok")]), &DebugValue::Counter(0));
    assert_eq!(samples(value(&snapshot, DURATION_SECONDS, &[])), 0);
    assert_eq!(samples(value(&snapshot, STAGE_SECONDS, &[("stage", "vfs")])), 0);
}

#[tokio::test]
async fn laps_and_finish_are_recorded() {
    let (metrics, snapshotter, _) = metrics();

    let mut timer = metrics.start();
    for stage in Stage::ALL {
        metrics.lap(&mut timer, stage);
    }
    metrics.finish(timer, true);
    metrics.finish(metrics.start(), false);

    let snapshot = snapshot(&snapshotter);

    assert_eq!(value(&snapshot, REQUESTS_TOTAL, &[("status", "ok")]), &DebugValue::Counter(1));
    assert_eq!(value(&snapshot, REQUESTS_TOTAL, &[("status", "error")]), &DebugValue::Counter(1));
    assert_eq!(samples(value(&snapshot, DURATION_SECONDS, &[])), 2);

    for stage in Stage::ALL {
        let value = value(&snapshot, STAGE_SECONDS, &[("stage", stage.as_str())]);
        assert_eq!(samples(value), 1, "stage {}", stage.as_str());
    }
}

#[tokio::test]
async fn in_flight_guard_restores_gauge() {
    let (metrics, snapshotter, recorder) = metrics();

    let _held = with_local_recorder(&recorder, || {
        let held = metrics.vfs_in_flight(0);
        let _dropped = metrics.vfs_in_flight(1);
        held
    });

    let snapshot = snapshot(&snapshotter);

    assert_eq!(value(&snapshot, VFS_IN_FLIGHT, &[("backend", "0")]), &gauge(1.0));
    assert_eq!(value(&snapshot, VFS_IN_FLIGHT, &[("backend", "1")]), &gauge(0.0));
}

#[tokio::test]
async fn tokio_metrics_are_reported_per_task_kind() {
    let (metrics, snapshotter, _) = metrics();

    for kind in TaskKind::ALL {
        metrics.instrument(kind, async {}).await;
    }
    // Lets the reporters, spawned in `NfsMetrics::new`, take their first sample.
    tokio::task::yield_now().await;

    let snapshot = snapshot(&snapshotter);

    for kind in TaskKind::ALL {
        assert_eq!(
            value(&snapshot, "nfsmamont_tokio_task_instrumented_count", &[("task", kind.as_str())]),
            &gauge(1.0),
            "task {}",
            kind.as_str()
        );
    }
}

#[tokio::test]
async fn write_reply() {
    let (metrics, snapshotter, _) = metrics();
    let (mut socket, io) = metrics.io_timer(Vec::new());

    metrics.write_reply(None, &io, socket.write_all(b"unmeasured")).await.unwrap();
    metrics.write_reply(Some(GETATTR), &io, socket.write_all(b"abcd")).await.unwrap();

    let snapshot = snapshot(&snapshotter);
    let labels = [("message", "reply"), ("proc", "getattr")];

    for name in [CPU_SECONDS, SYSCALL_SECONDS, WAIT_SECONDS] {
        assert_eq!(samples(value(&snapshot, name, &labels)), 1, "{name}");
    }
}

#[tokio::test]
async fn read_call() {
    let (metrics, snapshotter, _) = metrics();
    let (mut socket, io) = metrics.io_timer(&b"unmeasuredabcd"[..]);
    let mut buf = [0; 10];

    metrics.read_call(&io, socket.read_exact(&mut buf), |_| None).await.unwrap();
    metrics.read_call(&io, socket.read_exact(&mut buf[..4]), |_| Some(GETATTR)).await.unwrap();

    let snapshot = snapshot(&snapshotter);
    let labels = [("message", "call"), ("proc", "getattr")];

    for name in [CPU_SECONDS, SYSCALL_SECONDS] {
        assert_eq!(samples(value(&snapshot, name, &labels)), 1, "{name}");
    }
}
