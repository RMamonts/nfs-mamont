use std::future::poll_fn;
use std::num::NonZeroUsize;
use std::task::Poll;

use metrics_exporter_prometheus::{Matcher, PrometheusBuilder};

use super::*;
use crate::{Allocator, Impl};

fn recorder() -> metrics_exporter_prometheus::PrometheusRecorder {
    PrometheusBuilder::new()
        .set_buckets_for_metric(Matcher::Suffix("_seconds".into()), DURATION_BUCKETS)
        .unwrap()
        .build_recorder()
}

fn value(render: &str, name: &str, selector: &str) -> f64 {
    render
        .lines()
        .find(|line| line.starts_with(&format!("{name}{{")) && line.contains(selector))
        .unwrap_or_else(|| panic!("missing {name} matching {selector}:\n{render}"))
        .rsplit_once(' ')
        .unwrap()
        .1
        .parse()
        .unwrap()
}

#[test]
fn task_events_accumulate_but_statistics_replace_previous_sample() {
    let recorder = recorder();
    metrics::with_local_recorder(&recorder, || {
        let mut sample = TaskMetrics::default();
        sample.total_poll_count = 2;
        sample.total_poll_duration = Duration::from_millis(2);
        sample.max_idle_duration = Duration::from_millis(10);
        publish_task("test", "read", &sample);
        sample.max_idle_duration = Duration::from_millis(3);
        publish_task("test", "read", &sample);
        let render = recorder.handle().render();
        assert!(render.contains("# TYPE nfs_read_task_total_poll_count counter"));
        assert!(render.contains("# TYPE nfs_read_task_mean_poll_duration gauge"));
        assert_eq!(value(&render, "nfs_read_task_total_poll_count", "test"), 4.0);
        assert_eq!(value(&render, "nfs_read_task_total_poll_duration", "test"), 4000.0);
        assert_eq!(value(&render, "nfs_read_task_mean_poll_duration", "test"), 1000.0);
        assert_eq!(value(&render, "nfs_read_task_max_idle_duration", "test"), 3000.0);
        publish_task("test", "read", &TaskMetrics::default());
        let render = recorder.handle().render();
        assert_eq!(value(&render, "nfs_read_task_total_poll_count", "test"), 4.0);
        assert_eq!(value(&render, "nfs_read_task_mean_poll_duration", "test"), 0.0);
        assert_eq!(value(&render, "nfs_read_task_max_idle_duration", "test"), 0.0);
        assert_eq!(value(&render, "nfs_read_task_slow_poll_ratio", "test"), 0.0);
        assert_eq!(value(&render, "nfs_read_task_long_delay_ratio", "test"), 0.0);
    });
}

#[test]
fn runtime_totals_accumulate_and_worker_extrema_remain_interval_gauges() {
    let recorder = recorder();
    metrics::with_local_recorder(&recorder, || {
        let mut sample = RuntimeMetrics::default();
        sample.elapsed = Duration::from_secs(1);
        sample.workers_count = 4;
        sample.total_busy_duration = Duration::from_secs(2);
        sample.total_park_count = 10;
        sample.max_park_count = 5;
        sample.max_busy_duration = Duration::from_secs(1);
        publish_runtime("test", &sample);
        sample.max_park_count = 3;
        sample.max_busy_duration = Duration::from_millis(700);
        publish_runtime("test", &sample);
        let render = recorder.handle().render();
        assert_eq!(value(&render, "tokio_total_park_count", "test"), 20.0);
        assert_eq!(value(&render, "tokio_max_park_count", "test"), 3.0);
        assert_eq!(value(&render, "tokio_max_busy_duration", "test"), 700_000.0);
    });
}

#[test]
fn queue_sampling_observes_backlog_without_any_worker_dequeuing() {
    let recorder = recorder();
    metrics::with_local_recorder(&recorder, || {
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        runtime.block_on(async {
            let (tx, rx) = async_channel::unbounded();
            let _pool = PoolMetrics::new(&rx, 2);
            for number in 0..7 {
                tx.send(number).await.unwrap();
            }
            flush();
            assert_eq!(value(&recorder.handle().render(), "nfs_vfs_queue_depth", "pool="), 7.0);
            while rx.try_recv().is_ok() {}
            flush();
            assert_eq!(value(&recorder.handle().render(), "nfs_vfs_queue_depth", "pool="), 0.0);
            drop(rx);
            flush();
            assert_eq!(value(&recorder.handle().render(), "nfs_vfs_workers", "pool="), 0.0);
        });
    });
}

#[test]
fn allocator_reports_pending_immediately_and_cleans_up_cancelled_waits() {
    let recorder = recorder();
    metrics::with_local_recorder(&recorder, || {
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        runtime.block_on(async {
            let one = NonZeroUsize::new(1).unwrap();
            let allocator = Impl::new(one, one);
            let first = allocator.allocate(one).await.unwrap();
            let mut waiting = Box::pin(allocator.allocate(one));
            poll_fn(|cx| {
                assert!(waiting.as_mut().poll(cx).is_pending());
                Poll::Ready(())
            })
            .await;
            let render = recorder.handle().render();
            assert_eq!(value(&render, "nfs_allocator_waiters", "allocator="), 1.0);
            assert_eq!(value(&render, "nfs_allocator_waits_total", "allocator="), 1.0);
            drop(waiting);
            let render = recorder.handle().render();
            assert_eq!(value(&render, "nfs_allocator_waiters", "allocator="), 0.0);
            assert_eq!(value(&render, "nfs_allocator_waits_cancelled_total", "allocator="), 1.0);
            assert_eq!(
                value(&render, "nfs_allocator_wait_duration_seconds_count", "allocator="),
                1.0
            );
            let mut waiting = Box::pin(allocator.allocate(one));
            poll_fn(|cx| {
                assert!(waiting.as_mut().poll(cx).is_pending());
                Poll::Ready(())
            })
            .await;
            drop(first);
            let second = waiting.await.unwrap();
            assert_eq!(
                value(&recorder.handle().render(), "nfs_allocator_blocks_in_use", "allocator="),
                1.0
            );
            drop(second);
            let render = recorder.handle().render();
            assert_eq!(value(&render, "nfs_allocator_blocks_in_use", "allocator="), 0.0);
            assert_eq!(value(&render, "nfs_allocator_waiters", "allocator="), 0.0);
            assert_eq!(
                value(&render, "nfs_allocator_wait_duration_seconds_count", "allocator="),
                2.0
            );
            assert_eq!(value(&render, "nfs_allocator_waits_cancelled_total", "allocator="), 1.0);
        });
    });
}

#[test]
fn each_runtime_gets_fresh_reporters_and_its_own_task_counters() {
    let recorder = recorder();
    metrics::with_local_recorder(&recorder, || {
        for _ in 0..2 {
            let runtime =
                tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
            let id = runtime.handle().id().to_string();
            runtime.block_on(async {
                tokio::spawn(instrument_read(async { tokio::task::yield_now().await }))
                    .await
                    .unwrap();
                flush();
                let render = recorder.handle().render();
                assert_eq!(
                    value(
                        &render,
                        "nfs_read_task_instrumented_count",
                        &format!("runtime=\"{id}\"")
                    ),
                    1.0
                );
                assert_eq!(
                    value(&render, "nfs_read_task_live_count", &format!("runtime=\"{id}\"")),
                    0.0
                );
                assert!(
                    value(&render, "nfs_read_task_total_poll_count", &format!("runtime=\"{id}\""))
                        >= 2.0
                );
            });
        }
    });
}

#[test]
fn histogram_buckets_resolve_microsecond_durations_in_seconds() {
    let recorder = recorder();
    metrics::with_local_recorder(&recorder, || {
        metrics::histogram!("tokio_poll_time_histogram_seconds", "runtime" => "test")
            .record(0.000_05);
        let render = recorder.handle().render();
        assert_eq!(
            value(&render, "tokio_poll_time_histogram_seconds_bucket", "le=\"0.000025\""),
            0.0
        );
        assert_eq!(
            value(&render, "tokio_poll_time_histogram_seconds_bucket", "le=\"0.00005\""),
            1.0
        );
    });
}

#[cfg(tokio_unstable)]
#[test]
fn runtime_histogram_conversion_preserves_counts_and_uses_seconds() {
    let recorder = recorder();
    metrics::with_local_recorder(&recorder, || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .enable_metrics_poll_time_histogram()
            .metrics_poll_time_histogram_configuration(
                tokio::runtime::HistogramConfiguration::linear(Duration::from_micros(100), 100),
            )
            .build()
            .unwrap();
        let mut intervals = RuntimeMonitor::new(runtime.handle()).intervals();
        runtime.block_on(async {
            tokio::spawn(async {
                std::thread::sleep(Duration::from_millis(1));
            })
            .await
            .unwrap();
        });
        let sample = intervals.next().unwrap();
        let observations: u64 =
            sample.poll_time_histogram.buckets().iter().map(|bucket| bucket.count()).sum();
        assert!(observations >= 1);
        publish_runtime("histogram-test", &sample);
        let render = recorder.handle().render();
        assert_eq!(
            value(&render, "tokio_poll_time_histogram_seconds_count", "histogram-test"),
            observations as f64
        );
        let sum = value(&render, "tokio_poll_time_histogram_seconds_sum", "histogram-test");
        assert!(
            sum > 0.0 && sum <= observations as f64 * 0.01,
            "histogram sum should be seconds, got {sum}"
        );
    });
}

#[test]
fn vfs_durations_combine_procedures_and_workers_and_reset_each_interval() {
    let recorder = recorder();
    metrics::with_local_recorder(&recorder, || {
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        runtime.block_on(async {
            let (_tx, rx) = async_channel::unbounded::<()>();
            let pool = PoolMetrics::new(&rx, 2);
            let mut first = pool.worker(0);
            let mut second = pool.worker(1);
            first.record_elapsed("READ", Some(0), Duration::from_millis(10));
            first.record_elapsed("GETATTR", Some(1), Duration::from_millis(30));
            second.record_elapsed("READ", Some(0), Duration::from_millis(5));
            flush();
            let render = recorder.handle().render();
            assert!(
                (value(&render, "nfs_vfs_request_duration_seconds_mean", "pool=") - 0.015).abs()
                    < 1e-12
            );
            assert_eq!(value(&render, "nfs_vfs_request_duration_seconds_min", "pool="), 0.005);
            assert_eq!(value(&render, "nfs_vfs_request_duration_seconds_max", "pool="), 0.03);
            let counts: Vec<_> = render
                .lines()
                .filter(|line| line.starts_with("nfs_vfs_request_duration_seconds_count{"))
                .collect();
            assert_eq!(counts.len(), 1, "all procedures and workers share a duration histogram");
            assert!(counts[0].ends_with(" 3"));
            for line in
                render.lines().filter(|line| line.starts_with("nfs_vfs_request_duration_seconds"))
            {
                assert!(
                    !line.contains("operation=")
                        && !line.contains("worker=")
                        && !line.contains("backend="),
                    "duration labels: {line}"
                );
            }
            flush();
            let render = recorder.handle().render();
            for name in ["mean", "min", "max"] {
                assert!(value(
                    &render,
                    &format!("nfs_vfs_request_duration_seconds_{name}"),
                    "pool="
                )
                .is_nan());
            }
            second.record_elapsed("WRITE", Some(0), Duration::from_millis(20));
            flush();
            let render = recorder.handle().render();
            for name in ["mean", "min", "max"] {
                assert_eq!(
                    value(&render, &format!("nfs_vfs_request_duration_seconds_{name}"), "pool="),
                    0.02
                );
            }
        });
    });
}
