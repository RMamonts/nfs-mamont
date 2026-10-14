use crate::metric::nfs_metrics::{DURATION_SECONDS, REQUESTS_TOTAL, STAGE_SECONDS, VFS_IN_FLIGHT};
use crate::metric::tests::test_helpers::{samples, snapshot, value};
use crate::metric::{NfsMetrics, Stage, Timer};
use metrics::with_local_recorder;
use metrics_util::debugging::{DebugValue, DebuggingRecorder, Snapshotter};

pub(crate) mod test_helpers;
fn metrics() -> (NfsMetrics, Snapshotter) {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let metrics = with_local_recorder(&recorder, NfsMetrics::new);
    (metrics, snapshotter)
}

#[test]
fn timer_is_16_bytes() {
    assert_eq!(std::mem::size_of::<Timer>(), 16);
}

#[test]
fn unmeasured_timer_records_nothing() {
    let (metrics, snapshotter) = metrics();

    let mut timer = Timer::NONE;
    metrics.lap(&mut timer, Stage::Vfs);
    metrics.finish(timer, true);

    let snapshot = snapshot(&snapshotter);

    assert_eq!(value(&snapshot, REQUESTS_TOTAL, &[("status", "ok")]), &DebugValue::Counter(0));
    assert_eq!(samples(value(&snapshot, DURATION_SECONDS, &[])), 0);
    assert_eq!(samples(value(&snapshot, STAGE_SECONDS, &[("stage", "vfs")])), 0);
}

#[test]
fn laps_and_finish_are_recorded() {
    let (metrics, snapshotter) = metrics();

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

#[test]
fn in_flight_guard_restores_gauge() {
    let (metrics, snapshotter) = metrics();

    let _held = metrics.vfs_in_flight();
    let dropped = metrics.vfs_in_flight();
    drop(dropped);

    assert_eq!(value(&snapshot(&snapshotter), VFS_IN_FLIGHT, &[]), &DebugValue::Gauge(1.0.into()));
}
