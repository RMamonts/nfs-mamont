use std::num::NonZeroUsize;
use std::time::Duration;

use metrics::with_local_recorder;
use metrics_util::debugging::{DebugValue, DebuggingRecorder, Snapshotter};

use crate::allocator::Allocator as _;
use crate::allocator::Impl;
use crate::metric::allocator::allocator_metrics::{
    CHUNKS_CAPACITY, CHUNKS_IN_USE, REJECTED_TOTAL, REQUEST_BYTES, WAIT_SECONDS,
};
use crate::metric::tests::test_helpers::{samples, snapshot, value};

fn allocator(size: usize, count: usize) -> (Impl, Snapshotter) {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let allocator = with_local_recorder(&recorder, || {
        Impl::new(NonZeroUsize::new(size).unwrap(), NonZeroUsize::new(count).unwrap())
    });
    (allocator, snapshotter)
}

fn gauge(value: f64) -> DebugValue {
    DebugValue::Gauge(value.into())
}

#[tokio::test]
async fn allocation_and_release_are_recorded() {
    let (allocator, snapshotter) = allocator(4, 4);

    let released = allocator.allocate(NonZeroUsize::new(6).unwrap()).await.unwrap();
    let _held = allocator.allocate(NonZeroUsize::new(4).unwrap()).await.unwrap();
    drop(released);

    let snapshot = snapshot(&snapshotter);
    assert_eq!(value(&snapshot, CHUNKS_CAPACITY, &[]), &gauge(4.0));
    assert_eq!(value(&snapshot, CHUNKS_IN_USE, &[]), &gauge(1.0));
    assert_eq!(samples(value(&snapshot, WAIT_SECONDS, &[])), 2);
    assert_eq!(
        value(&snapshot, REQUEST_BYTES, &[]),
        &DebugValue::Histogram(vec![6.0.into(), 4.0.into()])
    );
}

#[tokio::test]
async fn blocked_allocation() {
    const DELAY: Duration = Duration::from_secs(1);
    let (allocator, snapshotter) = allocator(4, 4);

    let held = allocator.allocate(NonZeroUsize::new(16).unwrap()).await.unwrap();
    let release = async {
        tokio::time::sleep(DELAY).await;
        drop(held);
    };
    let (waited, ()) = tokio::join!(allocator.allocate(NonZeroUsize::new(4).unwrap()), release);
    assert!(waited.is_some());

    let snapshot = snapshot(&snapshotter);
    assert_eq!(value(&snapshot, CHUNKS_IN_USE, &[]), &gauge(1.0));
    let DebugValue::Histogram(wait) = value(&snapshot, WAIT_SECONDS, &[]) else {
        panic!("expected a histogram");
    };
    assert_eq!(wait.len(), 2);
    assert!(wait[1].0 >= DELAY.as_secs_f64(), "blocked wait {} is too short", wait[1]);
}

#[tokio::test]
async fn oversized_allocation_is_rejected_and_recorded() {
    let (allocator, snapshotter) = allocator(4, 4);

    assert!(allocator.allocate(NonZeroUsize::new(17).unwrap()).await.is_none());

    let snapshot = snapshot(&snapshotter);
    assert_eq!(value(&snapshot, REJECTED_TOTAL, &[]), &DebugValue::Counter(1));
    assert_eq!(value(&snapshot, CHUNKS_IN_USE, &[]), &gauge(0.0));
}
