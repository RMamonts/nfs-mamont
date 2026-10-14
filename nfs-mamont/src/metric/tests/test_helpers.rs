//! Helpers for reading metric values from a [`DebuggingRecorder`](metrics_util::debugging::DebuggingRecorder).

use metrics_util::debugging::{DebugValue, Snapshotter};
use metrics_util::CompositeKey;

/// Takes a snapshot of all metric values in debug recorder. Taking a snapshot resets the values
pub(crate) fn snapshot(snapshotter: &Snapshotter) -> Vec<(CompositeKey, DebugValue)> {
    snapshotter.snapshot().into_vec().into_iter().map(|(key, _, _, value)| (key, value)).collect()
}

/// Returns the value of metric `name` with exactly `labels`.
pub(crate) fn value<'a>(
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
pub(crate) fn samples(value: &DebugValue) -> usize {
    match value {
        DebugValue::Histogram(samples) => samples.len(),
        other => panic!("expected a histogram, got {other:?}"),
    }
}
