//! Converts tokio-metrics interval samples into explicit Prometheus semantics.
//! Event totals accumulate; sample statistics replace the preceding gauge value.

use std::collections::HashMap;
use std::future::{poll_fn, Future};
use std::pin::pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::{Duration, Instant};

use metrics::{Counter, Gauge, Histogram};
use tokio::runtime::{Handle, Id};
use tokio_metrics::{
    RuntimeIntervals, RuntimeMetrics, RuntimeMonitor, TaskIntervals, TaskMetrics, TaskMonitor,
};

pub(crate) const DURATION_BUCKETS: &[f64] = &[
    0.000_001,
    0.000_002_5,
    0.000_005,
    0.000_01,
    0.000_025,
    0.000_05,
    0.000_1,
    0.000_25,
    0.000_5,
    0.001,
    0.0025,
    0.005,
    0.01,
    0.025,
    0.05,
    0.1,
    0.25,
    0.5,
    1.0,
    2.5,
    5.0,
    10.0,
    30.0,
    60.0,
];

const REPORT_INTERVAL: Duration = Duration::from_secs(5);
static RUNTIMES: OnceLock<Mutex<HashMap<Id, Weak<Telemetry>>>> = OnceLock::new();
static NEXT_POOL: AtomicU64 = AtomicU64::new(0);
static NEXT_ALLOCATOR: AtomicU64 = AtomicU64::new(0);

type Sampler = Box<dyn Fn() -> bool + Send>;

struct Samples {
    runtime: RuntimeIntervals,
    tasks: [TaskIntervals; 3],
}

struct Telemetry {
    runtime: String,
    monitors: [TaskMonitor; 3],
    samples: Mutex<Samples>,
    samplers: Mutex<Vec<Sampler>>,
    read_cycles: Counter,
}

impl Telemetry {
    fn publish(&self) {
        // Serialize flushes and periodic collection so a delta is exported exactly once.
        let mut samples = self.samples.lock().unwrap();
        let runtime = samples.runtime.next().unwrap();
        publish_runtime(&self.runtime, &runtime);
        for (index, kind) in ["read", "write", "vfs"].into_iter().enumerate() {
            let sample = samples.tasks[index].next().unwrap();
            publish_task(&self.runtime, kind, &sample);
            let cumulative = self.monitors[index].cumulative();
            metrics::gauge!(format!("nfs_{kind}_task_live_count"), "runtime" => self.runtime.clone(), "task_kind" => kind)
                .set(cumulative.instrumented_count.saturating_sub(cumulative.dropped_count) as f64);
        }
        self.samplers.lock().unwrap().retain(|sample| sample());
    }
}

fn current() -> Arc<Telemetry> {
    let handle = Handle::current();
    let registry = RUNTIMES.get_or_init(Mutex::default);
    let mut runtimes = registry.lock().unwrap();
    if let Some(telemetry) = runtimes.get(&handle.id()).and_then(Weak::upgrade) {
        return telemetry;
    }
    runtimes.retain(|_, telemetry| telemetry.strong_count() != 0);
    let runtime = handle.id().to_string();
    let monitors = std::array::from_fn(|_| TaskMonitor::new());
    let samples = Samples {
        runtime: RuntimeMonitor::new(&handle).intervals(),
        tasks: std::array::from_fn(|index| monitors[index].intervals()),
    };
    let telemetry = Arc::new(Telemetry {
        read_cycles: metrics::counter!("nfs_rpc_read_cycles_total", "runtime" => runtime.clone()),
        runtime,
        monitors,
        samples: Mutex::new(samples),
        samplers: Mutex::default(),
    });
    runtimes.insert(handle.id(), Arc::downgrade(&telemetry));
    let reporter = telemetry.clone();
    handle.spawn(async move {
        let mut interval = tokio::time::interval(REPORT_INTERVAL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            reporter.publish();
        }
    });
    telemetry
}

pub(crate) fn start() {
    let _ = current();
}

pub(crate) fn flush() {
    current().publish();
}

fn instrument<F: Future>(future: F, index: usize) -> impl Future<Output = F::Output> {
    // Instrument before spawn, so first-poll delay includes scheduling the new task.
    current().monitors[index].instrument(future)
}

pub(crate) fn instrument_read<F: Future>(future: F) -> impl Future<Output = F::Output> {
    instrument(future, 0)
}
pub(crate) fn instrument_write<F: Future>(future: F) -> impl Future<Output = F::Output> {
    instrument(future, 1)
}
pub(crate) fn instrument_vfs<F: Future>(future: F) -> impl Future<Output = F::Output> {
    instrument(future, 2)
}

pub(crate) struct ReadMetrics(Counter);
impl ReadMetrics {
    pub(crate) fn new() -> Self {
        Self(current().read_cycles.clone())
    }
    pub(crate) fn record_cycle(&self) {
        self.0.increment(1);
    }
}

fn micros(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000_000.0
}

fn ratio(numerator: u64, denominator: u64) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

fn publish_task(runtime: &str, kind: &'static str, m: &TaskMetrics) {
    let labels = [("runtime", runtime.to_owned()), ("task_kind", kind.to_owned())];
    let gauge = |name: &str| metrics::gauge!(format!("nfs_{kind}_task_{name}"), &labels);
    macro_rules! counters {
        ($($field:ident),* $(,)?) => {$(
            metrics::counter!(format!("nfs_{kind}_task_{}", stringify!($field)), &labels).increment(m.$field);
        )*};
    }
    macro_rules! durations {
        ($($field:ident),* $(,)?) => {$(
            metrics::counter!(format!("nfs_{kind}_task_{}", stringify!($field)), &labels)
                .increment(m.$field.as_micros().min(u64::MAX as u128) as u64);
        )*};
    }
    macro_rules! means {
        ($($method:ident),* $(,)?) => {$(
            gauge(stringify!($method)).set(micros(m.$method()));
        )*};
    }
    counters!(
        instrumented_count,
        dropped_count,
        first_poll_count,
        total_idled_count,
        total_scheduled_count,
        total_poll_count,
        total_fast_poll_count,
        total_slow_poll_count,
        total_short_delay_count,
        total_long_delay_count
    );
    durations!(
        total_first_poll_delay,
        total_idle_duration,
        total_scheduled_duration,
        total_poll_duration,
        total_fast_poll_duration,
        total_slow_poll_duration,
        total_short_delay_duration,
        total_long_delay_duration
    );
    means!(
        mean_first_poll_delay,
        mean_idle_duration,
        mean_scheduled_duration,
        mean_poll_duration,
        mean_fast_poll_duration,
        mean_slow_poll_duration,
        mean_short_delay_duration,
        mean_long_delay_duration
    );
    gauge("max_idle_duration").set(micros(m.max_idle_duration));
    gauge("slow_poll_ratio").set(ratio(m.total_slow_poll_count, m.total_poll_count));
    gauge("long_delay_ratio").set(ratio(m.total_long_delay_count, m.total_scheduled_count));
    gauge("slow_poll_threshold_seconds")
        .set(TaskMonitor::DEFAULT_SLOW_POLL_THRESHOLD.as_secs_f64());
    gauge("long_delay_threshold_seconds")
        .set(TaskMonitor::DEFAULT_LONG_DELAY_THRESHOLD.as_secs_f64());
}

fn publish_runtime(runtime: &str, m: &RuntimeMetrics) {
    macro_rules! gauges {
        ($($field:ident),* $(,)?) => {$(
            metrics::gauge!(concat!("tokio_", stringify!($field)), "runtime" => runtime.to_owned()).set(m.$field as f64);
        )*};
    }
    macro_rules! counters {
        ($($field:ident),* $(,)?) => {$(
            metrics::counter!(concat!("tokio_", stringify!($field)), "runtime" => runtime.to_owned()).increment(m.$field);
        )*};
    }
    macro_rules! duration_gauges {
        ($($field:ident),* $(,)?) => {$(
            metrics::gauge!(concat!("tokio_", stringify!($field)), "runtime" => runtime.to_owned()).set(micros(m.$field));
        )*};
    }
    gauges!(workers_count, live_tasks_count, max_park_count, min_park_count, global_queue_depth);
    counters!(total_park_count);
    duration_gauges!(max_busy_duration, min_busy_duration);
    metrics::counter!("tokio_total_busy_duration", "runtime" => runtime.to_owned())
        .increment(m.total_busy_duration.as_micros().min(u64::MAX as u128) as u64);
    metrics::gauge!("tokio_report_interval_seconds", "runtime" => runtime.to_owned())
        .set(m.elapsed.as_secs_f64());
    #[cfg(tokio_unstable)]
    {
        gauges!(
            max_noop_count,
            min_noop_count,
            max_steal_count,
            min_steal_count,
            max_steal_operations,
            min_steal_operations,
            max_local_schedule_count,
            min_local_schedule_count,
            max_overflow_count,
            min_overflow_count,
            max_polls_count,
            min_polls_count,
            total_local_queue_depth,
            max_local_queue_depth,
            min_local_queue_depth,
            blocking_queue_depth,
            blocking_threads_count,
            idle_blocking_threads_count
        );
        counters!(
            total_noop_count,
            total_steal_count,
            total_steal_operations,
            num_remote_schedules,
            total_local_schedule_count,
            total_overflow_count,
            total_polls_count,
            budget_forced_yield_count,
            io_driver_ready_count
        );
        duration_gauges!(
            mean_poll_duration,
            mean_poll_duration_worker_min,
            mean_poll_duration_worker_max
        );
        metrics::gauge!("tokio_mean_polls_per_park", "runtime" => runtime.to_owned())
            .set(m.mean_polls_per_park());
        let histogram = metrics::histogram!("tokio_poll_time_histogram_seconds", "runtime" => runtime.to_owned());
        for bucket in m.poll_time_histogram.buckets() {
            if bucket.count() > 0 {
                // Tokio exposes buckets, not individual observations. Preserve their
                // counts using the lower bound as an explicitly documented approximation.
                histogram.record_many(bucket.range_start().as_secs_f64(), bucket.count() as usize);
            }
        }
    }
}

#[derive(Clone, Copy)]
pub struct Timestamp(Instant);

impl Timestamp {
    pub(crate) fn now() -> Self {
        Self(Instant::now())
    }
}

pub(crate) struct PoolMetrics {
    runtime: String,
    pool: String,
    duration: Histogram,
    durations: Vec<Arc<Mutex<DurationSummary>>>,
}

#[derive(Default)]
struct DurationSummary {
    count: u64,
    total: Duration,
    min: Option<Duration>,
    max: Duration,
}

impl DurationSummary {
    fn record(&mut self, elapsed: Duration) {
        self.count = self.count.saturating_add(1);
        self.total = self.total.saturating_add(elapsed);
        self.min = Some(self.min.map_or(elapsed, |minimum| minimum.min(elapsed)));
        self.max = self.max.max(elapsed);
    }

    fn merge(&mut self, other: Self) {
        self.count = self.count.saturating_add(other.count);
        self.total = self.total.saturating_add(other.total);
        if let Some(minimum) = other.min {
            self.min = Some(self.min.map_or(minimum, |current| current.min(minimum)));
        }
        self.max = self.max.max(other.max);
    }
}

struct DurationGauges {
    mean: Gauge,
    min: Gauge,
    max: Gauge,
}

impl DurationGauges {
    fn publish(&self, sample: &DurationSummary) {
        // An empty interval has no measured duration; zero would be a false minimum.
        if sample.count == 0 {
            self.mean.set(f64::NAN);
            self.min.set(f64::NAN);
            self.max.set(f64::NAN);
        } else {
            self.mean.set(sample.total.as_secs_f64() / sample.count as f64);
            self.min.set(sample.min.unwrap().as_secs_f64());
            self.max.set(sample.max.as_secs_f64());
        }
    }
}

impl PoolMetrics {
    pub(crate) fn new<T: Send + 'static>(
        receiver: &async_channel::Receiver<T>,
        workers: usize,
    ) -> Self {
        let telemetry = current();
        let pool = NEXT_POOL.fetch_add(1, Ordering::Relaxed).to_string();
        let runtime = telemetry.runtime.clone();
        let labels = [("runtime", runtime.clone()), ("pool", pool.clone())];
        let duration = metrics::histogram!("nfs_vfs_request_duration_seconds", &labels);
        let duration_gauges = DurationGauges {
            mean: metrics::gauge!("nfs_vfs_request_duration_seconds_mean", &labels),
            min: metrics::gauge!("nfs_vfs_request_duration_seconds_min", &labels),
            max: metrics::gauge!("nfs_vfs_request_duration_seconds_max", &labels),
        };
        duration_gauges.publish(&DurationSummary::default());
        let durations: Vec<_> =
            (0..workers).map(|_| Arc::new(Mutex::new(DurationSummary::default()))).collect();
        let sampled_durations = durations.clone();
        let queue = metrics::gauge!("nfs_vfs_queue_depth", "runtime" => runtime.clone(), "pool" => pool.clone());
        let worker_count = metrics::gauge!("nfs_vfs_workers", "runtime" => runtime.clone(), "pool" => pool.clone());
        worker_count.set(workers as f64);
        queue.set(receiver.len() as f64);
        let receiver = receiver.downgrade();
        telemetry.samplers.lock().unwrap().push(Box::new(move || {
            let mut summary = DurationSummary::default();
            for worker in &sampled_durations {
                summary.merge(std::mem::take(&mut *worker.lock().unwrap()));
            }
            duration_gauges.publish(&summary);
            if let Some(receiver) = receiver.upgrade() {
                queue.set(receiver.len() as f64);
                true
            } else {
                queue.set(0.0);
                worker_count.set(0.0);
                false
            }
        }));
        Self { runtime, pool, duration, durations }
    }

    pub(crate) fn worker(&self, worker: usize) -> WorkerMetrics {
        let labels = vec![
            ("runtime", self.runtime.clone()),
            ("pool", self.pool.clone()),
            ("worker", worker.to_string()),
        ];
        let in_flight = metrics::gauge!("nfs_vfs_requests_in_flight", &labels);
        // Include idle workers in pool min/max, even before their first request.
        in_flight.set(0.0);
        WorkerMetrics {
            queue_wait: metrics::histogram!("nfs_vfs_queue_wait_duration_seconds", &labels),
            in_flight,
            duration: self.duration.clone(),
            durations: self.durations[worker].clone(),
            labels,
            requests: HashMap::new(),
        }
    }
}

pub(crate) struct WorkerMetrics {
    labels: Vec<(&'static str, String)>,
    queue_wait: Histogram,
    in_flight: Gauge,
    requests: HashMap<(&'static str, Option<u8>), Counter>,
    duration: Histogram,
    durations: Arc<Mutex<DurationSummary>>,
}

impl WorkerMetrics {
    pub(crate) fn begin(&self, queued_at: Timestamp) -> RequestGuard {
        self.queue_wait.record(queued_at.0.elapsed().as_secs_f64());
        self.in_flight.increment(1.0);
        RequestGuard(self.in_flight.clone())
    }

    pub(crate) fn record_request(
        &mut self,
        operation: &'static str,
        backend: Option<u8>,
        started: Timestamp,
    ) {
        self.record_elapsed(operation, backend, started.0.elapsed());
    }

    fn record_elapsed(&mut self, operation: &'static str, backend: Option<u8>, elapsed: Duration) {
        let count = self.requests.entry((operation, backend)).or_insert_with(|| {
            let mut labels = self.labels.clone();
            labels.push(("operation", operation.to_owned()));
            labels
                .push(("backend", backend.map_or_else(|| "none".to_owned(), |id| id.to_string())));
            metrics::counter!("nfs_vfs_requests_total", &labels)
        });
        count.increment(1);
        self.duration.record(elapsed.as_secs_f64());
        self.durations.lock().unwrap().record(elapsed);
    }
}

pub(crate) struct RequestGuard(Gauge);
impl Drop for RequestGuard {
    fn drop(&mut self) {
        self.0.decrement(1.0);
    }
}

#[derive(Debug)]
pub(crate) struct AllocatorMetrics {
    acquired: Counter,
    returned: Counter,
    in_use: Gauge,
    capacity: Gauge,
    waits: Counter,
    waiters: Gauge,
    wait_duration: Histogram,
    cancelled: Counter,
}

impl AllocatorMetrics {
    pub(crate) fn new(blocks: usize) -> Self {
        let id = NEXT_ALLOCATOR.fetch_add(1, Ordering::Relaxed).to_string();
        let labels = [("allocator", id)];
        let metrics = Self {
            acquired: metrics::counter!("nfs_allocator_blocks_acquired_total", &labels),
            returned: metrics::counter!("nfs_allocator_blocks_returned_total", &labels),
            in_use: metrics::gauge!("nfs_allocator_blocks_in_use", &labels),
            capacity: metrics::gauge!("nfs_allocator_capacity_blocks", &labels),
            waits: metrics::counter!("nfs_allocator_waits_total", &labels),
            waiters: metrics::gauge!("nfs_allocator_waiters", &labels),
            wait_duration: metrics::histogram!("nfs_allocator_wait_duration_seconds", &labels),
            cancelled: metrics::counter!("nfs_allocator_waits_cancelled_total", &labels),
        };
        metrics.capacity.set(blocks as f64);
        metrics.in_use.set(0.0);
        metrics.waiters.set(0.0);
        metrics
    }

    pub(crate) async fn acquire<F: Future>(&self, future: F) -> F::Output {
        let mut future = pin!(future);
        let mut wait = WaitGuard { metrics: self, started: None, finished: false };
        let result = poll_fn(|cx| {
            let result = future.as_mut().poll(cx);
            if result.is_pending() && wait.started.is_none() {
                wait.started = Some(Instant::now());
                self.waits.increment(1);
                self.waiters.increment(1.0);
            }
            result
        })
        .await;
        wait.finished = true;
        result
    }

    pub(crate) fn allocate(&self, blocks: usize) {
        self.acquired.increment(blocks as u64);
        self.in_use.increment(blocks as f64);
    }

    pub(crate) fn release(&self, blocks: usize) {
        self.returned.increment(blocks as u64);
        self.in_use.decrement(blocks as f64);
    }
}

impl Drop for AllocatorMetrics {
    fn drop(&mut self) {
        self.capacity.set(0.0);
    }
}

struct WaitGuard<'a> {
    metrics: &'a AllocatorMetrics,
    started: Option<Instant>,
    finished: bool,
}

impl Drop for WaitGuard<'_> {
    fn drop(&mut self) {
        if let Some(started) = self.started {
            self.metrics.wait_duration.record(started.elapsed().as_secs_f64());
            self.metrics.waiters.decrement(1.0);
            if !self.finished {
                self.metrics.cancelled.increment(1);
            }
        }
    }
}

#[cfg(test)]
mod tests;
