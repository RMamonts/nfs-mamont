# Tokio and server metrics

Enable the optional `metrics` Cargo feature to publish runtime and task
measurements through the `metrics` crate:

```toml
nfs-mamont = { path = "../nfs-mamont", features = ["metrics"] }
```

The default build has no metrics dependencies, reporter tasks, poll monitoring,
or timestamp measurements. Use this build for a comparison without instrumentation.
It does not install a global recorder by itself. Applications may install any
`metrics` recorder before creating allocators or `ServerContext`; the helper
`install_prometheus_exporter` starts a Prometheus HTTP scrape endpoint.

## Start Prometheus and Grafana

The repository includes a local Prometheus and Grafana stack with a provisioned
datasource and dashboard. Start it from the repository root:

```sh
docker compose -f docker-compose.metrics.yml up -d
```

Configure your application to install the exporter before creating allocators,
server contexts, or starting NFS traffic. For example:

```rust,no_run
use std::net::SocketAddr;

fn install_metrics() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    nfs_mamont::install_prometheus_exporter(SocketAddr::from(([0, 0, 0, 0], 9000)))?;
    Ok(())
}
```

Call `install_metrics()` first, then construct the allocator and `ServerContext`,
then call `handle_forever(...).await`. Metric handles are cached; installing or
replacing a recorder after these resources exist is unsupported.
Keep the exporter port reachable from the Prometheus container. The supplied
compose file targets `host.docker.internal:9000`; on Linux it adds the host-gateway
mapping automatically.

To validate the real server path, run the example in another terminal:

```sh
cargo run -p nfs-mamont --features metrics --example metrics_smoke -- 30
```

It starts a real NFS server on a temporary loopback TCP port and four clients
that pipeline READ and GETATTR calls. The small backend delays successful READs
by 25 ms and returns normal NFS errors for GETATTR. This exercises the actual
read/write tasks, VFS dispatch, shared queue, and a four-block allocator.
It checks reply statuses and prints completed RPC throughput. No OS mount is
needed. The exporter uses port 9000. After the workload drains, the example
flushes metrics and keeps the exporter alive for another 11 seconds for scrapes.

The example is a controlled validation workload, not a backend performance
benchmark. To estimate instrumentation overhead on your machine, build the same
example in release mode with and without the feature, set
`NFS_METRICS_BACKEND_DELAY_US=0`, and compare repeated equal-duration runs:

```sh
NFS_METRICS_BACKEND_DELAY_US=0 cargo run --release -p nfs-mamont --no-default-features --example metrics_smoke -- 30
NFS_METRICS_BACKEND_DELAY_US=0 cargo run --release -p nfs-mamont --features metrics --example metrics_smoke -- 30
```

Use the printed workload throughput, which excludes the final scrape grace
period. Run only one copy at a time. Repeat the comparison with your real backend
and load pattern before drawing performance conclusions.

Open Grafana at <http://localhost:3000>. The dashboard is provisioned in the
**NFS Mamont** folder. Prometheus is also available at
<http://localhost:9091>. Anonymous Grafana access is enabled for local development;
do not expose this compose stack to an untrusted network.

The dashboard has one chart per independently collapsible row. All 15 rows start
collapsed; click a row header to show or hide its chart and explanation. Each
expanded chart uses the full dashboard width and a larger plot height, with no
side-by-side charts. Row titles identify Runtime, Tasks, VFS, or Allocator.
Legends show task kinds, operations, pools, workers, or allocators;
target addresses and runtime IDs are omitted from the legend text. The queries
still preserve those labels, so distinct series are not merged. For multiple
targets/runtimes with the same displayed legend, use panel inspection to identify
their complete labels.

Each chart has a visible explanatory header giving its meaning, units,
aggregation window, and source metric names from the tables below. Header text
is displayed by a Markdown Text panel directly above the associated chart so it
also works with the bundled Grafana 12 classic dashboard format. The chart's
description additionally includes its exact PromQL queries for inspection.

If you imported the JSON manually into a separate Grafana installation, import
the updated `monitoring/grafana/dashboards/nfs-mamont.json` again and overwrite
the existing dashboard. Editing the repository file alone does not update a
manually imported copy. Provisioned installations load the file automatically.

### Using an existing Prometheus and Grafana installation

Importing the dashboard does not configure Prometheus to collect NFS metrics.
If Prometheus runs directly on the NFS host, add this job under `scrape_configs`
in its active configuration file and reload that configuration:

```yaml
  - job_name: nfs-mamont
    scrape_interval: 5s
    scrape_timeout: 5s
    static_configs:
      - targets: [localhost:9000]
```

Use the NFS host address instead of `localhost` when Prometheus runs elsewhere.
The `host.docker.internal:9000` address in the bundled configuration is for the
Docker collector. While the example is running with `--features metrics`, check
Prometheus **Status → Target health**: the `nfs-mamont` target must be UP. A
configuration that scrapes only `localhost:9090` collects Prometheus's own metrics
and contains no NFS history.

In a separately installed Grafana, configure a Prometheus data source using the
actual collector address (typically `http://localhost:9090` for a native collector
on the Grafana host), make it the default, and import
`monitoring/grafana/dashboards/nfs-mamont.json`. Choose a time range covering the
run. The bundled Docker collector instead publishes port `9091` on its host.

Stop the monitoring stack with:

```sh
docker compose -f docker-compose.metrics.yml down
```

Prometheus writes to the named Docker volume `nfs-mamont-prometheus-data` and
keeps samples for 90 days. `docker compose down` removes the containers but keeps
this volume. Do not use `docker compose down -v` if you want to keep the history.
Choose a retention period long enough to cover the run and the time before you
copy it to the analysis machine.

## Review a run later on another machine

Prometheus can collect and store the series on the NFS host without Grafana being
open. To move the collected history to another machine, stop Prometheus cleanly
after the run, archive its data volume, then copy and restore that archive on the
analysis machine. The compose file pins the Prometheus image version so both
machines use the same TSDB reader.

On the NFS host, from the repository directory:

```sh
docker compose -f docker-compose.metrics.yml up -d prometheus
```

This starts only the collector; Grafana does not need to run on the NFS host.
With the `metrics` feature, `handle_forever` automatically calls
`flush_metrics()` after its accept loop stops and then waits 11 seconds before
returning. Keep Prometheus running throughout this grace period. For the bundled
five-second scrape interval, this allows two final scrapes while the runtime and
exporter are still alive; the application does not need another delay after
`handle_forever(...).await` returns.

Stop the workload and drain client requests before shutting down the server.
Independently spawned connection and VFS tasks may still be running during the
grace period, so it is not a drain barrier. Cancelling/aborting `handle_forever`
or forcibly terminating the process bypasses the grace period. A scrape still
depends on Prometheus being reachable and healthy; the delay is not an
acknowledgement that Prometheus persisted the sample.

For manual collection on other runtimes, or if your scrape interval is longer
than five seconds, publish the final samples inside each runtime and keep its
exporter alive for at least two of your scrape intervals. `flush_metrics()`
publishes to the in-memory recorder; it cannot force an external scrape:

```rust,no_run
# async fn finish_run() {
nfs_mamont::flush_metrics();
tokio::time::sleep(std::time::Duration::from_secs(11)).await;
# }
```

Archive the data after this final collection:

```sh
docker compose -f docker-compose.metrics.yml stop prometheus
docker run --rm \
  -v nfs-mamont-prometheus-data:/data:ro \
  -v "$PWD:/backup" \
  alpine:3.22 \
  tar -C /data -czf /backup/nfs-mamont-prometheus.tar.gz .
scp nfs-mamont-prometheus.tar.gz analyst@workstation:/path/to/metrics/
```

Copy the archive after Prometheus has stopped. It contains the TSDB blocks and
the WAL, which includes recent samples not yet compacted into blocks. Prometheus
recommends snapshots for live backups; stopping first avoids copying files while
they are changing.

On the analysis machine, copy the archive into the repository directory. Restore
it into a fresh named volume before starting the stack. If that volume already
contains a Prometheus database, remove it first while no Prometheus container is
using it; the archive is a complete database and should replace the old volume,
not be merged into it.

```sh
docker volume create nfs-mamont-prometheus-data
docker run --rm \
  -v nfs-mamont-prometheus-data:/data \
  -v "$PWD:/backup:ro" \
  alpine:3.22 \
  tar -C /data -xzf /backup/nfs-mamont-prometheus.tar.gz
docker compose -f docker-compose.metrics.yml up -d
```

Open Grafana at <http://localhost:3000>, select a time range covering the
recording period, and open the provisioned **NFS Mamont / Tokio runtime**
dashboard. The compose stack may show the scrape target as down on the analysis
machine because the original NFS host is absent; the archived samples remain
queryable for their recorded time range. Keep the Prometheus retention period
long enough that it does not expire the samples before the archive is made.

## Time resolution and collection cost

Duration precision and the interval between chart samples are different:

| Layer | Current behavior | What it controls |
| --- | --- | --- |
| Instrumentation | Measures each monitored poll, VFS request, and allocator wait as it happens | Duration observations can be below one millisecond even with a five-second scrape interval. |
| Tokio reporter and VFS queue sampling | Every 5 seconds (`REPORT_INTERVAL` in `src/telemetry/enabled.rs`) | Publishes Tokio interval statistics and snapshots shared VFS queue depth. |
| Prometheus scrape | Every 5 seconds in the supplied NFS job | Copies the current exporter state into the historical database. |
| Grafana refresh | Every 5 seconds while viewing | Repeats queries against Prometheus; it does not initiate an NFS exporter scrape. |
| Dashboard aggregation windows | 1 minute for counter rates/task means; 5 minutes for VFS queue-wait p95; latest reporter interval for VFS handling mean/min/max | Determines which observations contribute to each displayed statistic. |

For example, a 2 ms VFS request is recorded with that duration immediately.
Prometheus can collect its histogram bucket/count/sum later; it does not have to
scrape every 2 ms to include the observation. A histogram retains the distribution
within the interval, rather than the timestamp and identity of every request.
The 1 µs minimum exporter bucket boundary also does not make this a 1 µs sampled
timeline. Tokio idle/scheduling durations become observable on subsequent polls,
as described below.

To see queue or task-statistic changes on a finer timeline, shorten both the
reporter interval and the Prometheus scrape interval. Shortening only the scrape
interval repeatedly collects unchanged Tokio samples and queue snapshots.
Short-lived queue bursts between reporter snapshots can still be missed.
The current reporter interval is a code constant, not a runtime setting.
Changing Grafana refresh, the displayed duration unit, or the query step cannot
recover observations that were never sampled. Query windows also need to be
short enough for the phenomenon being investigated, but long enough to contain
multiple scrape observations for rates and enough requests for useful quantiles.

For backend comparison runs, start with **1 second** for both reporter and scrape
intervals if the current five-second timeline is too coarse. For a short targeted
investigation of queue bursts, consider **100 ms** and a limited set of series.
These are starting points to validate under load, not measured overhead limits.
A full Prometheus scrape every **1–10 ms** is a poor default for this collector:

| Interval | Snapshots per second | Frequency versus the current 5-second interval |
| --- | ---: | ---: |
| 5 s | 0.2 | 1× |
| 1 s | 1 | 5× |
| 100 ms | 10 | 50× |
| 10 ms | 100 | 500× |
| 1 ms | 1000 | 5000× |

These factors multiply publication/scrape frequency and the approximate number of
stored samples, not the CPU usage of the whole NFS server. If there are `S`
exported series and the scrape interval is `T` seconds, Prometheus receives about
`S / T` samples per second. Histogram buckets each contribute a series.

The NFS process pays for reading monitor atomics, converting and publishing
samples, waking the reporter, and formatting the entire metrics HTTP response.
The exporter shares the server's runtime, CPU, and memory bandwidth. Prometheus
additionally pays for receiving, parsing, and storing the samples. Per-poll and
per-request instrumentation costs already occur at event time and are not
multiplied merely by changing the scrape interval. There is no defensible fixed
percentage slowdown without an A/B measurement using the real backend, the same
load, the same enabled series, and a separately measured scrape duration/size.
The instrumentation guidance also recommends benchmarking performance-critical
paths: <https://prometheus.io/docs/practices/instrumentation/#inner-loops>.

For a genuine 1–10 ms event timeline, a separate recording mode that buffers
selected events/samples locally and exports them in batches after the run is a
more suitable design than scraping all series over HTTP at that frequency. Its
clock, buffering, storage, and event instrumentation costs also need measurement;
it is not implemented by the current Prometheus collector.

The dashboard layout changes do not alter any collection intervals. The default
continues to be five seconds. Keep the shutdown scrape grace period longer than
the configured scrape interval; the bundled 11-second grace covers the current
five-second job.

## Collection semantics and labels

Reporters start lazily when the first VFS pool or instrumented task is created
on a runtime. There is one reporter per Tokio runtime, identified by a `runtime`
label. A new runtime receives a new reporter even after an earlier runtime has
shut down. Task groups belong to the runtime where they are spawned; runtime
metrics cover that runtime, including application and monitoring tasks.

Every five seconds, the reporter reads `tokio-metrics` intervals and publishes:

- **Counter:** accumulated event counts or duration sums since this collector
  started. Use `rate()` or `increase()`, including for names ending in `_count`.
- **Gauge:** a current state or a statistic of the latest reporter interval.
  Read directly; do not apply `rate()` to means, ratios, or worker extrema.
- **Histogram:** cumulative observations with `_bucket`, `_count`, and `_sum`.
  Histograms ending in `_seconds` use seconds.

Tokio task/runtime duration counters and duration gauges without a `_seconds`
suffix use **microseconds**. Duration counters add each interval's duration sum,
not its average. A counter may reset when a process/collector restarts. Tokio
task-group interval statistics are zero when there are no new observations. VFS
handling mean/min/max are NaN for empty intervals, displayed as gaps rather than
zero-duration requests. `flush_metrics()` creates an additional, possibly short,
reporter interval. Prefer counter-based weighted means over multiple intervals.

The implementation uses `TaskMonitor` and `RuntimeMonitor` directly. It does not
use the `tokio-metrics 0.5.2` metrics-rs reporter, whose counter/gauge mapping
would give different semantics for several similarly named series.

`runtime`, `pool`, and `allocator` IDs are process-local identifiers, not stable
backend names. VFS pools also have a `pool` label, worker measurements add
`worker`, and completed-request counters add `operation` and `backend` (backend registry
slot, or `none` for calls without a single backend). Allocator measurements have
an `allocator` label and may cover use across multiple runtimes. VFS handling
duration measurements aggregate all procedures, backends, and workers within
each runtime/pool and have no operation, backend, or worker label. Task groups
have `runtime` and `task_kind` labels; there are no per-connection labels.

To compare backends across separate runs, assign a run/backend label in the
Prometheus scrape target configuration, for example:

```yaml
static_configs:
  - targets: [host.docker.internal:9000]
    labels:
      run: backend-a-001
```

Change this label for each independent recording and filter Grafana queries by
it as needed. Backend slot IDs can be reused after detaching a backend, so use
separate run labels or time ranges when replacing a backend.

**Compatibility:** earlier versions of this implementation exported interval
counts as gauges and accumulated some means as counters. Their samples cannot
be interpreted with the corrected queries. Keep old recordings in a separate
run/target series and avoid query windows spanning the change. The runtime poll
histogram is now named `tokio_poll_time_histogram_seconds`.

## Runtime metrics

The complete stable set is below. Every metric has a `runtime` label. Worker
extrema are extrema across workers in the latest interval, not lifetime extrema.

| Metric | Type / unit | Meaning |
| --- | --- | --- |
| `tokio_workers_count` | Gauge | Runtime worker threads; 1 for a current-thread runtime. |
| `tokio_live_tasks_count` | Gauge | Live Tokio tasks on this runtime, including application and monitoring tasks. |
| `tokio_max_park_count` | Gauge | Highest number of parks by one worker in the latest interval. |
| `tokio_min_park_count` | Gauge | Lowest number of parks by one worker in the latest interval. |
| `tokio_total_park_count` | Counter | Accumulated worker parks. |
| `tokio_total_busy_duration` | Counter / µs | Accumulated busy wall time across all workers. |
| `tokio_max_busy_duration` | Gauge / µs | Largest worker busy duration in the latest interval. |
| `tokio_min_busy_duration` | Gauge / µs | Smallest worker busy duration in the latest interval. |
| `tokio_global_queue_depth` | Gauge | Runnable tasks in the runtime global queue at collection. |
| `tokio_report_interval_seconds` | Gauge / seconds | Elapsed runtime measurement interval; useful when flushing between periodic samples. |

The following additional metrics require `tokio_unstable` in the application and
all dependencies:

```sh
RUSTFLAGS="--cfg tokio_unstable" cargo run -p nfs-mamont --features metrics --example metrics_smoke -- 30
```

The runtime poll histogram additionally requires
`enable_metrics_poll_time_histogram()` on the application's Tokio runtime builder.
The library cannot change an already constructed runtime. The supplied example
uses the default runtime builder and does not enable that optional histogram.

| Metric | Type / unit | Meaning |
| --- | --- | --- |
| `tokio_mean_poll_duration` | Gauge / µs | Mean task poll time reported by Tokio across workers. |
| `tokio_mean_poll_duration_worker_min` | Gauge / µs | Lowest worker mean poll time reported by Tokio. |
| `tokio_mean_poll_duration_worker_max` | Gauge / µs | Highest worker mean poll time reported by Tokio. |
| `tokio_poll_time_histogram_seconds` | Histogram / seconds | Approximate poll distribution, populated only when enabled on the runtime builder. |
| `tokio_total_noop_count` | Counter | Accumulated wakeups that found no work before parking across workers. |
| `tokio_max_noop_count` | Gauge | Highest number of wakeups that found no work before parking on one worker in the latest interval. |
| `tokio_min_noop_count` | Gauge | Lowest number of wakeups that found no work before parking on one worker in the latest interval. |
| `tokio_total_steal_count` | Counter | Accumulated tasks stolen from other workers across workers. |
| `tokio_max_steal_count` | Gauge | Highest number of tasks stolen from other workers on one worker in the latest interval. |
| `tokio_min_steal_count` | Gauge | Lowest number of tasks stolen from other workers on one worker in the latest interval. |
| `tokio_total_steal_operations` | Counter | Accumulated successful steal operations across workers. |
| `tokio_max_steal_operations` | Gauge | Highest number of successful steal operations on one worker in the latest interval. |
| `tokio_min_steal_operations` | Gauge | Lowest number of successful steal operations on one worker in the latest interval. |
| `tokio_total_local_schedule_count` | Counter | Accumulated local scheduling events across workers. |
| `tokio_max_local_schedule_count` | Gauge | Highest number of local scheduling events on one worker in the latest interval. |
| `tokio_min_local_schedule_count` | Gauge | Lowest number of local scheduling events on one worker in the latest interval. |
| `tokio_total_overflow_count` | Counter | Accumulated local queue overflows across workers. |
| `tokio_max_overflow_count` | Gauge | Highest number of local queue overflows on one worker in the latest interval. |
| `tokio_min_overflow_count` | Gauge | Lowest number of local queue overflows on one worker in the latest interval. |
| `tokio_total_polls_count` | Counter | Accumulated task polls across workers. |
| `tokio_max_polls_count` | Gauge | Highest number of task polls on one worker in the latest interval. |
| `tokio_min_polls_count` | Gauge | Lowest number of task polls on one worker in the latest interval. |
| `tokio_num_remote_schedules` | Counter | Accumulated tasks scheduled from outside this runtime. |
| `tokio_total_local_queue_depth` | Gauge | Current runnable tasks across all worker local queues. |
| `tokio_max_local_queue_depth` | Gauge | Largest current local queue depth on one worker. |
| `tokio_min_local_queue_depth` | Gauge | Smallest current local queue depth on one worker. |
| `tokio_blocking_queue_depth` | Gauge | Tasks waiting for the blocking thread pool. |
| `tokio_blocking_threads_count` | Gauge | Additional blocking pool threads. |
| `tokio_idle_blocking_threads_count` | Gauge | Idle blocking pool threads. |
| `tokio_budget_forced_yield_count` | Counter | Accumulated yields forced by cooperative budget exhaustion. |
| `tokio_io_driver_ready_count` | Counter | Accumulated readiness events processed by the I/O driver. |
| `tokio_mean_polls_per_park` | Gauge | Interval task polls divided by parks minus no-op parks; zero if the denominator is zero. |

Tokio's worker mean poll time is a Tokio-provided statistic, rather than a
reconstruction of individual poll samples by this library. The poll histogram
re-exports Tokio bucket counts using each original bucket's lower bound as a
representative observation, converted to seconds. Its sum and quantiles are
therefore approximations with the original bucket resolution; they are not
exact individual poll timings. Configure the Tokio histogram resolution as well
as the exporter buckets if you need precise percentiles.

## Instrumented task groups

Read tasks, write tasks, and VFS workers have separate `TaskMonitor`s per runtime.
Every row below expands into three series names by replacing `{task}` with
`read`, `write`, and `vfs`. For example,
`nfs_{task}_task_total_poll_count` means `nfs_read_task_total_poll_count`,
`nfs_write_task_total_poll_count`, and `nfs_vfs_task_total_poll_count`.

| Metric name | Type / unit | Meaning |
| --- | --- | --- |
| `nfs_{task}_task_instrumented_count` | Counter | Tasks instrumented since collection began. |
| `nfs_{task}_task_dropped_count` | Counter | Instrumented tasks dropped, whether completed, failed, or cancelled. |
| `nfs_{task}_task_live_count` | Gauge | Instrumented tasks minus dropped tasks, from cumulative monitor values. |
| `nfs_{task}_task_first_poll_count` | Counter | Tasks polled for the first time. |
| `nfs_{task}_task_total_first_poll_delay` | Counter / µs | Summed delays from instrumentation to first poll. |
| `nfs_{task}_task_total_idled_count` | Counter | Completed idle intervals observed on a subsequent poll; not a count of all Pending returns. |
| `nfs_{task}_task_total_idle_duration` | Counter / µs | Summed completed idle intervals, from the end of a poll until a later wakeup. |
| `nfs_{task}_task_max_idle_duration` | Gauge / µs | Longest completed idle interval observed in the latest reporter interval. |
| `nfs_{task}_task_total_scheduled_count` | Counter | Scheduling intervals observed when a woken task is next polled. |
| `nfs_{task}_task_total_scheduled_duration` | Counter / µs | Summed delays from wakeup until subsequent poll. |
| `nfs_{task}_task_total_poll_count` | Counter | Calls to poll monitored tasks. |
| `nfs_{task}_task_total_poll_duration` | Counter / µs | Summed wall time inside monitored polls. |
| `nfs_{task}_task_total_fast_poll_count` | Counter | Polls below the slow-poll threshold. |
| `nfs_{task}_task_total_fast_poll_duration` | Counter / µs | Summed duration of fast polls. |
| `nfs_{task}_task_total_slow_poll_count` | Counter | Polls at or above the slow-poll threshold. |
| `nfs_{task}_task_total_slow_poll_duration` | Counter / µs | Summed duration of slow polls. |
| `nfs_{task}_task_total_short_delay_count` | Counter | Scheduling intervals below the long-delay threshold. |
| `nfs_{task}_task_total_long_delay_count` | Counter | Scheduling intervals at or above the long-delay threshold. |
| `nfs_{task}_task_total_short_delay_duration` | Counter / µs | Summed short scheduling delays. |
| `nfs_{task}_task_total_long_delay_duration` | Counter / µs | Summed long scheduling delays. |
| `nfs_{task}_task_mean_first_poll_delay` | Gauge / µs | Mean first-poll delay in the latest interval. |
| `nfs_{task}_task_mean_idle_duration` | Gauge / µs | Mean completed idle interval in the latest interval. |
| `nfs_{task}_task_mean_scheduled_duration` | Gauge / µs | Mean scheduling delay in the latest interval. |
| `nfs_{task}_task_mean_poll_duration` | Gauge / µs | Mean poll duration in the latest interval. |
| `nfs_{task}_task_slow_poll_ratio` | Gauge / 0–1 | Slow polls divided by all polls in the latest interval. |
| `nfs_{task}_task_long_delay_ratio` | Gauge / 0–1 | Long scheduling delays divided by all scheduling delays in the latest interval. |
| `nfs_{task}_task_mean_fast_poll_duration` | Gauge / µs | Mean fast poll duration in the latest interval. |
| `nfs_{task}_task_mean_short_delay_duration` | Gauge / µs | Mean short scheduling delay in the latest interval. |
| `nfs_{task}_task_mean_slow_poll_duration` | Gauge / µs | Mean slow poll duration in the latest interval. |
| `nfs_{task}_task_mean_long_delay_duration` | Gauge / µs | Mean long scheduling delay in the latest interval. |
| `nfs_{task}_task_slow_poll_threshold_seconds` | Gauge / seconds | TaskMonitor default slow-poll threshold: 50 µs on non-WASM platforms. |
| `nfs_{task}_task_long_delay_threshold_seconds` | Gauge / seconds | TaskMonitor default long scheduling delay: 50 µs on non-WASM platforms. |

These thresholds classify observations; they do not establish an application
latency objective. On WASM the tokio-metrics defaults are 500 ms.

Idle and scheduling measurements are retrospective: a task still waiting has
not contributed its entire ongoing wait. Self-waking tasks can return Pending
without an idle interval. A task that never wakes again may never report that
last idle interval. These measurements do not represent instantaneous occupancy
of all futures in each state. A single poll can process multiple RPC requests.

## Server and allocator metrics

`R` means the `runtime` label, `P` adds `pool`, and `W` adds `worker`.
`O` adds `operation` and `backend`. `A` means the `allocator` label only.
All counters include successful and error completions where applicable; no error
status labels or procedure-specific parser/serializer timings are collected.

| Metric | Labels | Type / unit | Meaning |
| --- | --- | --- | --- |
| `nfs_rpc_read_cycles_total` | R | Counter | Completed read/parse attempts, including errors and EOF. Cancelled unfinished attempts are not completions. |
| `nfs_vfs_requests_total` | W+O | Counter | VFS dispatches completed with a normal success or error response. |
| `nfs_vfs_request_duration_seconds` | P | Histogram / seconds | Combined dispatch/backend durations for all procedures, backends, and workers in the pool. Before response construction and sending to the writer; includes synchronous error logging. |
| `nfs_vfs_request_duration_seconds_mean` | P | Gauge / seconds | Total handling duration divided by completed request count in the latest reporter interval, combined across all workers/procedures. NaN if empty. |
| `nfs_vfs_request_duration_seconds_min` | P | Gauge / seconds | Shortest completed handling duration in the latest reporter interval, from actual observations. NaN if empty. |
| `nfs_vfs_request_duration_seconds_max` | P | Gauge / seconds | Longest completed handling duration in the latest reporter interval, from actual observations. NaN if empty. |
| `nfs_vfs_queue_wait_duration_seconds` | W | Histogram / seconds | Time from enqueue attempt to worker dispatch, including receiver scheduling delay. Recorded only when dequeued. |
| `nfs_vfs_queue_depth` | P | Gauge | Shared queue length sampled every reporter interval, independently of dequeues. There is one series per pool, not per worker. |
| `nfs_vfs_workers` | P | Gauge | Configured pool worker count while the pool receiver exists; zero after receiver cleanup. This is not a liveness/health check. |
| `nfs_vfs_requests_in_flight` | W | Gauge | Requests held by each worker, including handing the reply to the writer; cancellation decrements the gauge. |
| `nfs_allocator_blocks_acquired_total` | A | Counter | Blocks successfully handed to buffers by the built-in allocator. |
| `nfs_allocator_blocks_returned_total` | A | Counter | Blocks returned to that allocator. |
| `nfs_allocator_blocks_in_use` | A | Gauge | Blocks held by allocated buffers. |
| `nfs_allocator_capacity_blocks` | A | Gauge | Allocator capacity in blocks; zero after its final owner is dropped. |
| `nfs_allocator_waits_total` | A | Counter | Acquisitions whose semaphore future actually returned Pending; incremented on the first Pending. |
| `nfs_allocator_waiters` | A | Gauge | Acquisitions currently waiting for semaphore completion. |
| `nfs_allocator_waits_cancelled_total` | A | Counter | Pending acquisitions dropped before completion. |
| `nfs_allocator_wait_duration_seconds` | A | Histogram / seconds | First Pending to Ready or cancellation, including scheduler delay. Excludes buffer assembly after semaphore acquisition. |

Allocator metrics cover `Impl`, not arbitrary user implementations of `Allocator`.
Install the recorder before constructing `Impl`, including if constructing it
outside a runtime. The in-use/capacity metrics refer to blocks, not bytes.
Only successful allocations acquire blocks; oversized requests rejected before
semaphore acquisition do not increment block or wait counters.

The helper configures only histograms ending in `_seconds`, with bucket bounds
(in seconds):

```text
0.000001, 0.0000025, 0.000005, 0.00001, 0.000025, 0.00005,
0.0001, 0.00025, 0.0005, 0.001, 0.0025, 0.005, 0.01, 0.025, 0.05,
0.1, 0.25, 0.5, 1, 2.5, 5, 10, 30, 60, +Inf
```

The finite range is 1 µs–60 s. Percentiles are bucket estimates; observations
above 60 seconds cannot be resolved further. Applications may install their own
recorder with different per-metric buckets.

## Queries and interpretation

Polls per second, separated by runtime:

```promql
rate(nfs_read_task_total_poll_count[1m])
```

Weighted mean poll duration in microseconds over five minutes:

```promql
rate(nfs_read_task_total_poll_duration[5m])
/
rate(nfs_read_task_total_poll_count[5m])
```

Do the same with scheduled duration/count or idle duration/count. Do not average
interval means across workers or intervals: their observation counts differ.
Zero counts make weighted means undefined; an empty result/NaN during an idle
window is expected, rather than a measured zero-duration operation.

The VFS handling chart shows three lines per runtime/pool, combining procedures,
backends, and workers. All three refer to requests recorded in the same latest
reporter interval (normally five seconds):

```promql
nfs_vfs_request_duration_seconds_mean
nfs_vfs_request_duration_seconds_min
nfs_vfs_request_duration_seconds_max
```

The mean is computed from total duration and count, with each request carrying
equal weight. Min/max use actual measured durations, not histogram bucket
estimates. Per-worker summaries are merged and reset at publication. An empty
interval produces a gap. Like other interval gauges, a published interval can be
missed if no scrape captures it before the next publication. The cumulative
histogram retains observations for longer-window mean/distribution queries.
Older histogram-only recordings cannot recover exact interval minima/maxima.
If comparing backend implementations, record separate runs or pools because
handling duration no longer has a backend or operation label.

For a longer-window mean, use the cumulative histogram:

```promql
rate(nfs_vfs_request_duration_seconds_sum[1m])
/
rate(nfs_vfs_request_duration_seconds_count[1m])
```

Allocator pressure is visible directly in `nfs_allocator_waiters` and in the
ratio of blocks in use to capacity. Wait-duration histograms include completed
or cancelled waits, so pair them with the current waiter gauge to see ongoing
stalls. The VFS throughput spread shows whether workers complete requests at
different rates; worker-level poll statistics remain aggregated by task kind.

### VFS pool totals and worker spread

VFS charts that previously drew a line for every worker now show two series per
runtime/pool, independent of the number of workers. The spread is the maximum
worker value minus the minimum worker value at each point. It is not the largest
difference over the entire recording.

| Chart | Overall value | Worker spread |
| --- | --- | --- |
| In-flight requests | Sum of requests currently held by all workers. | Maximum worker in-flight count minus minimum worker count. Usually 0 or 1, since a worker holds at most one request. |
| Throughput | Sum of all worker completion rates over a 1-minute window, in requests/s. | Maximum worker rate minus minimum worker rate over the same window. Idle workers with no completion series contribute zero. |
| Queue wait | p95 of the combined histogram across all workers over 5 minutes. | Maximum worker p95 minus minimum worker p95 over the same window. Workers without observed waits are excluded; an empty pool produces a gap. |

The queue-wait overall value is computed from the pooled histogram buckets, not
by summing or averaging worker percentiles. With only one observed worker its
spread is zero. A worker here is a VFS worker task, not an OS thread. Worker
labels remain in exported measurements so Prometheus can compute these
aggregates, but the dashboard legends do not list individual workers.

For example, the two in-flight queries are:

```promql
sum by (instance, job, run, runtime, pool) (nfs_vfs_requests_in_flight)
```

```promql
max by (instance, job, run, runtime, pool) (nfs_vfs_requests_in_flight)
-
min by (instance, job, run, runtime, pool) (nfs_vfs_requests_in_flight)
```

Throughput queries fill missing worker rates with zero using the initialized
in-flight gauge, so workers that have not handled a request are included in the
minimum. Queue-wait spread filters undefined worker quantiles rather than
treating a missing latency observation as zero. Runtime, pool, run, job, and
instance labels are kept during aggregation to avoid mixing independent pools.

Rising scheduling queues and long task scheduling delays indicate scarce
runtime worker time. Synchronous blocking and OS scheduling can also produce
this pattern, so it is not proof of CPU saturation. Long idle intervals can
reflect I/O, channel waits, allocator waits, or an idle client. Poll duration is
wall time inside poll, including blocking and OS preemption, not CPU time.

## Limits and validation

Queue depth is a point sample every five seconds. It observes persistent backlog
even when all workers are blocked, but can miss bursts entirely between samples.
The depth of a shared queue must not be summed across duplicate worker snapshots.
Request duration excludes response serialization and network delivery and is not
end-to-end client latency. NFS NULL calls bypass VFS dispatch and therefore do
not contribute to VFS request counters.

Read/write monitoring wraps the whole long-lived task. Socket-specific Pending
and idle time is not separated from other awaits. Child tasks spawned by a
backend contribute to runtime task counts when using that runtime, but are not
attributed to a particular parent request; inline child futures are not separate
Tokio tasks. Panicked/cancelled dispatches do not increment completed-request
counters, although task-drop and in-flight metrics still reflect their cleanup.

Poll monitoring adds clocks, atomics, and wake instrumentation. Labels and
metric handles for workers/allocator operations are cached to avoid creating
strings on every request. The first use of an operation/backend combination
registers its request-counter handle. Exact handling-duration statistics use a
short lock per worker, shared with the reporter, to keep count/sum/min/max
consistent. Reporter conversion and queue sampling run every five
seconds. Actual overhead depends on poll frequency, concurrency, backend cost,
and the recorder; the feature-disabled build removes instrumentation.

Regression tests check cumulative counts versus interval statistics,
queue backlog with no dequeues, real allocator contention and wait
cancellation, multiple runtime lifetimes, histogram bucket units, and combined
VFS duration statistics including empty/reset intervals. Run them:

```sh
cargo test -p nfs-mamont --features metrics telemetry
RUSTFLAGS="--cfg tokio_unstable" cargo test -p nfs-mamont --features metrics telemetry
cargo check -p nfs-mamont --no-default-features --all-targets
```

The real-server example verifies successful and failed RPC replies and exercises
all pipeline stages. A successful scrape or Grafana query alone does not prove
measurement correctness. Use the controlled workload and the tests together.
Prometheus stores scraped observations only; neither the exporter nor a final
flush can recover a past gauge history that was never scraped.
