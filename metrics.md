# Server metrics

The library records metrics through the [`metrics`](https://docs.rs/metrics)
facade when the `metrics` Cargo feature is enabled.

The library does not ship an exporter. The application installs a `metrics`
recorder, for example `metrics-exporter-prometheus`, **before** calling
`ServerContext::new` and constructing the built-in allocator:
metric handles are registered there and stay bound to the recorder installed at
that moment. The exception is `nfsmamont_vfs_in_flight`, registered on each
backend's first request in the recorder installed at that time.

## Request pipeline

A request is timed from the moment it is parsed through three stages (vfs queue, vfs, reply queue) to the moment its reply is written.

Only NFSv3 calls are measured.

| Metric                           | Type      | Labels                                      | Unit  | Meaning                                                                                                                                   |
| -------------------------------- | --------- | ------------------------------------------- | ----- | ----------------------------------------------------------------------------------------------------------------------------------------- |
| `nfsmamont_rpc_stage_seconds`    | histogram | `stage` = `vfs_queue`, `vfs`, `reply_queue` | s     | Time spent in one pipeline stage; parsing the call and writing the reply is covered by the message metrics                                |
| `nfsmamont_rpc_duration_seconds` | histogram | —                                           | s     | Parsed -> reply written                                                                                                                   |
| `nfsmamont_rpc_requests_total`   | counter   | `status` = `ok`, `error`                    | count | Completed requests; `error` is an internal server error or a failed reply write                                                           |
| `nfsmamont_vfs_in_flight`        | gauge     | `backend` = backend id                      | count | Requests executing in a backend                                                                                                           |
| `nfsmamont_queue_depth`          | gauge     | `queue` = `vfs`, `reply`                    | count | Messages waiting in a channel: `vfs` is the VFS pool queue, `reply` sums the per-connection reply queues (MOUNT and NLM replies included) |

## Message metrics

Both connection tasks run (de)serialization under a `tokio_metrics::FutureMonitor` with their socket half wrapped in `IoTimer`, which sums the time spent in and bytes moved by the socket syscalls.

| Metric                          | Type      | Labels                              | Unit  | Meaning                                              |
| ------------------------------- | --------- | ----------------------------------- | ----- | ---------------------------------------------------- |
| `nfsmamont_rpc_cpu_seconds`     | histogram | `proc`, `message` = `call`, `reply` | s     | Poll time minus syscall time: (de)serialization      |
| `nfsmamont_rpc_syscall_seconds` | histogram | `proc`, `message` = `call`, `reply` | s     | Time in socket calls (mostly syscall time)           |
| `nfsmamont_rpc_wait_seconds`    | histogram | `proc`, `message` = `reply`         | s     | Idle time: waiting for the socket to become writable |
| `nfsmamont_rpc_reply_bytes`     | histogram | `proc`, `message` = `reply`         | bytes | Bytes written, record marking included               |

Nothing is recorded for a reply whose write failed.

Read-side caveats:

- The parser reads into a 2500-byte buffer, so one `read` can pull in several calls; the first carries the syscall time of all of them.
  CPU per call is exact; syscall time should be averaged over a time interval.
- There is no read-side wait: idle time there could be the client not sending.
- READ and WRITE call CPU time includes allocating the data buffer.

## Allocator metrics

The built-in allocator records its own metrics.

| Metric                                | Type      | Labels | Unit  | Meaning                                        |
| ------------------------------------- | --------- | ------ | ----- | ---------------------------------------------- |
| `nfsmamont_allocator_wait_seconds`    | histogram | —      | s     | Time an allocation waits for free chunks       |
| `nfsmamont_allocator_request_bytes`   | histogram | —      | bytes | Requested size of served allocations           |
| `nfsmamont_allocator_rejected_total`  | counter   | —      | count | Allocations larger than the allocator capacity |
| `nfsmamont_allocator_chunks_in_use`   | gauge     | —      | count | Chunks lent out                                |
| `nfsmamont_allocator_chunks_capacity` | gauge     | —      | count | Total chunks, set once at construction         |
| `nfsmamont_allocator_chunks_size`     | gauge     | —      | bytes | Chunk size, set once at construction           |

## [Tokio metrics](https://docs.rs/tokio-metrics)

Read and Write tasks use `TaskMonitor` (1 per task kind, 2 total) to instrument each worker.

A call to `ServerContext::new` also spawns task reporters that periodically (`TOKIO_SAMPLING_INTERVAL`) sample `TaskMonitor`'s metrics to any configured recorder.

| Source                                                                                            | Name                          | Labels                   |
| ------------------------------------------------------------------------------------------------- | ----------------------------- | ------------------------ |
| [`TaskMetrics`](https://docs.rs/tokio-metrics/latest/tokio_metrics/struct.TaskMetrics.html)       | `nfsmamont_tokio_task_<name>` | `task` = `read`, `write` |

## Clocksource

> [!CAUTION]

All of these read the clock several times per message.
Kernel clocksource other than `tsc` could increase the measurement overhead
(check `/sys/devices/system/clocksource/clocksource0/current_clocksource`).

## Example exporter setup

Setting up Prometheus listener and recorder with native histograms (exponential buckets).

```toml
metrics-exporter-prometheus = { version = "0.18.3", features = ["protobuf"]} # Also requires protobuf compiler to be installed
```

```rust
fn init_metrics_exporter(port: u16) {
    use metrics_exporter_prometheus::NativeHistogramConfig;
    metrics_exporter_prometheus::PrometheusBuilder::new()
        .set_native_histogram_for_metric(
            metrics_exporter_prometheus::Matcher::Prefix("nfsmamont_".to_string()),
            {
                let bucket_factor = 2.0;
                let max_buckets = 32;
                let zero_threshold = 0.0;
                NativeHistogramConfig::new(bucket_factor, max_buckets, zero_threshold).unwrap()
            },
        )
        .with_http_listener(([0, 0, 0, 0], port))
        .install()
        .expect("failed to install Prometheus recorder");
}
```
