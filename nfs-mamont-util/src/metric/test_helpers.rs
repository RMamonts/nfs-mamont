//! Helpers for reading metric values from a [`DebuggingRecorder`](metrics_util::debugging::DebuggingRecorder).

use metrics::with_local_recorder;
use metrics_util::debugging::{DebugValue, DebuggingRecorder, Snapshotter};
use metrics_util::CompositeKey;

/// Builds `T` with a fresh debugging recorder installed, so its metric handles are bound to it.
pub fn with_recorder<T>(build: impl FnOnce() -> T) -> (T, Snapshotter, DebuggingRecorder) {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let value = with_local_recorder(&recorder, build);
    (value, snapshotter, recorder)
}

/// Takes a snapshot of all metric values in debug recorder. Taking a snapshot resets the values
pub fn snapshot(snapshotter: &Snapshotter) -> Vec<(CompositeKey, DebugValue)> {
    snapshotter.snapshot().into_vec().into_iter().map(|(key, _, _, value)| (key, value)).collect()
}

/// Returns the value of metric `name` with exactly `labels`.
pub fn value<'a>(
    snapshot: &'a [(CompositeKey, DebugValue)],
    name: &str,
    labels: &[(&str, &str)],
) -> &'a DebugValue {
    snapshot
        .iter()
        .find(|(key, ..)| {
            let key = key.key();
            let key_labels: Vec<_> = key.labels().map(|l| (l.key(), l.value())).collect();
            key.name() == name && key_labels == labels
        })
        .map(|(_, value)| value)
        .unwrap_or_else(|| panic!("metric {name} {labels:?} not found"))
}

/// Count samples in a histogram
pub fn samples(value: &DebugValue) -> usize {
    match value {
        DebugValue::Histogram(samples) => samples.len(),
        other => panic!("expected a histogram, got {other:?}"),
    }
}

/// Returns the value of gauge `name` with exactly `labels`.
pub fn gauge_value(
    snapshot: &[(CompositeKey, DebugValue)],
    name: &str,
    labels: &[(&str, &str)],
) -> f64 {
    match value(snapshot, name, labels) {
        DebugValue::Gauge(value) => value.into_inner(),
        other => panic!("expected a gauge, got {other:?}"),
    }
}
