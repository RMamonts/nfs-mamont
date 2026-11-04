# Global budgets, the DRR scheduler and the allocator as the last resort: experiments

Subject: the branch `claude/gifted-euler-xcjo3a` = `main` @ `70d688a` + PR #450 (ported, 8 commits)
+ the implementation of the proposal in `docs/design/admission-control-and-qos.md`. Baseline:
`main` @ `70d688a`. Date: 2026-11-04. Data: `data/results.jsonl` (139 records), the log
`data/run.log`, the tables `data/summary.md`.

## TL;DR

| Question | Answer |
|---|---|
| What do the mechanisms cost? | Within the noise of the stand (±5–15 %). One at a time (E2b, 4 connections): `max_pending` 0 %, the byte budget 0 %, the write timeout −3 % (a `tokio::time::timeout` per reply), DRR **+4…+6 %**. On one connection at depth 1 DRR gives **+30 % at 42 % CPU instead of 60 %**: waking a worker through `Notify` is cheaper than through `async_channel`. |
| An upper bound for memory? | A client that does not read its replies: `main` 1476 MiB in 15 s, with the limits 4.5 MiB (E5). A write timeout of 5 s closes such a connection (`write_timeouts=1`). |
| Did the allocator stop being the back-pressure? | Yes. Next to a client that sends READ 1 MiB and never reads, a victim with READ 4 KiB gets **59–61k ops/s instead of 0** (E6). In every run with the budget the allocator waited a handful of times and milliseconds in total; without the budget in read:4k ×4 it waited 92 401 times and 18.7 s in total (E4), and read:4k ×4 itself got **+23 %** faster. |
| QoS for every client? | A light client next to a heavy one (E7): p50 34.8 ms on `main`, 4.5 ms with the limit of 256, **1.4 ms with DRR (quantum 8)** while the heavy one keeps its 57k ops/s. The limit of 64 gives the same 1.3 ms but costs the heavy one −19 %. |
| Many connections? | 1024 connections × 16 in flight, a probe of depth 1 (E8): probe p50 **278 → 22 ms**, with 256 connections 70 → 9 ms; the throughput of 58k ops/s does not change. This comes from `max_pending` with round-robin admission; DRR adds nothing here. |
| Does the fast one get more without starving the slow one? | Fast (depth 256) + slow (depth 2), E9: the fast one gets ~57k ops/s in every variant, there is no equal split. With DRR the slow one additionally gets **1131 ops/s instead of 450** and a p50 of 1.7 ms instead of 4.2. |
| Cost by bytes? | Metadata next to a deep queue of READ 1 MiB on 32 workers (E10b): FIFO gives the metadata a p50 of **110 ms** and 580 ops/s, DRR **16 ms** and 4300 ops/s; the stream stays at the device ceiling of 2048 MiB/s. |

## 1. The stand

- A VM, 4 vCPU Intel Xeon 2.1 GHz, 16 GiB. The server on CPUs 0–1 (`taskset`, two tokio worker
  threads), the load on CPUs 2–3, loopback. Rust toolchain 1.97, MSRV 1.75 checked
  (`cargo +1.75.0 test`).
- The mock VFS and the `nfs-bench` generator from the branch `vyar/mock_vfs`, adapted to the
  current API (`mock-vfs/` in this branch); `slow-reader` from the report
  `research/in-flight-limit`.
- Defaults: 64 VFS workers, allocator 64 × 1 MiB, zero backend latency. Three runs per point,
  the median; VM noise ±5–15 %, large differences are confirmed by the counters, not by ops/s
  alone.
- Server variants (`bench/bench.py`):

| Variant | Binary | Flags |
|---|---|---|
| `main` | `server-main` (`main` @ `70d688a`) | no limits |
| `inflight` | `server-exp` | `--max-in-flight 256` (PR #450 as is) |
| `budgets` | `server-exp` | inflight + `--max-pending auto` (= 256) `--max-connections 4096` `--data-budget auto` (7/8 of the pool = 56 MiB) `--max-data-per-conn 32M` `--write-timeout 30s`, FIFO |
| `drr` | `server-exp` | budgets + `--scheduler drr:32` |
| `pending`, `wtimeout`, `databudget`, `drr-only` | `server-exp` | inflight + one mechanism |

The server prints the counters of `nfs_mamont::stats` on stop: `in_flight_waits` (parks on the
connection slots), `pending_waits`, `data_budget_waits`, `conn_data_waits`, `allocator_waits`
and `allocator_wait_ms` (the allocator itself waited), `write_timeouts`, `sched_rotations`.

## 2. The cost of the mechanisms

### E1. One connection, GETATTR, backend without latency, ops/s

| depth | `main` | `inflight` | `budgets` | `drr` |
|---|---|---|---|---|
| 1 | 15.3k (CPU 60 %) | 16.3k | 15.9k | **20.3k (CPU 42 %)** |
| 32 | 74.2k | 71.5k | 68.8k | 69.9k |
| 256 | 85.4k | 111.2k | 106.0k | 112.8k |
| 1024 | 84.1k | 142.9k | 135.4k | 131.8k |

- While the limit is not reached (depth ≤ 32) every variant is within noise; the dip of
  `budgets` at depth 32 (−7 %) does not reproduce in E2b and lies within the spread between
  series (`inflight` gave 291k in E2 and 274k in E2b at the same point).
- When the limit is reached the gain of +30…+70 % repeats §4.1 of the `in-flight-limit`
  report: the socket becomes the queue and the read task reads full buffers.
- At depth 1 DRR is 30 % faster at lower CPU. The reason is not scheduling (one flow, nothing
  to share) but the wake-up: a DRR worker parks on `tokio::sync::Notify` and is woken by one
  `notify_one`, while the workers of the FIFO pool register as listeners of `async_channel`
  (event-listener), which is noticeably more expensive per request.

### E2 and E2b. Four connections, GETATTR, ops/s

| depth | `main` | `inflight` | `budgets` | `drr` | `pending` | `wtimeout` | `databudget` | `drr-only` |
|---|---|---|---|---|---|---|---|---|
| 64 | 299k | 291k / 274k | 259k | 275k | 276k | 267k | 274k | **290k** |
| 512 | 286k | 325k / 322k | 303k | 325k | 333k | 311k | 329k | **333k** |

(`inflight` is given from two series, E2 / E2b.) One mechanism at a time: `max_pending` and the
byte budget cost nothing (`try_acquire` without waiting, one `release` per request), the write
timeout costs ~3 % (a timer registration per reply), DRR gives +4…+6 %. Conclusion: the write
timeout should be redone as a per-connection watchdog (one check per second, zero work per
request); the rest can be enabled by default.

### E4. Data path, one connection (four for read:4k)

| Load | `main` | `inflight` | `budgets` | `drr` |
|---|---|---|---|---|
| read:64k d32 | 22.2k ops/s | 21.8k | 21.7k | 22.6k |
| read:1m d16 | 3165 MiB/s | 3006 | 3026 | 3078 |
| write:64k d32 | 23.2k ops/s (±11 %) | 20.6k | 20.2k | 22.1k (±20 %) |
| read:4k ×4 d64 | 177k ops/s | 179k (the allocator waited 92 401 times, 18.7 s) | **219k** (the budget waited 54 092 times, the allocator 7 times, 2.9 ms) | **217k** |

read:4k ×4 is bound by the pool: 256 READs in flight, each taking a whole 1 MiB buffer out of
64. When the wait moves from the allocator to the budget (56 units out of 64), a request admitted
by the budget always finds a free buffer and is parsed without a second park; there are half as
many parks and the throughput grows by 23 %. A side effect, but a telling one: the allocator is
no longer the waiting point.

## 3. Memory and the allocator

### E5. A client sends GETATTR and does not read the replies, 15 s

| Server | Accepted requests | RSS at the end | RSS every 4 s |
|---|---|---|---|
| `main` | 5 010 944 | **1476 MiB** | 120 → 583 → 944 → 1296 |
| `inflight` | 95 180 | 4.5 MiB | 4 → 4 → 4 → 4 |
| `budgets` | 94 685 | 4.7 MiB | 4 → 4 → 4 → 4 |
| `budgets --write-timeout 5s` | 95 180 | 4.6 MiB, `write_timeouts=1`: the connection is closed | 4 → 4 → 4 |

### E6. A client sends READ 1 MiB and does not read; the victim does read:4k at depth 16

| Server | Victim alone | Victim next to the hog | Counters |
|---|---|---|---|
| `main` | 61.9k ops/s | **0** (stalled, not one reply in 15 s) | |
| `inflight` | 57.3k | **0** (stalled) | the allocator waited 33 s in total |
| `budgets` | 59.6k | **58.9k** | `conn_data_waits=3`, the allocator did not wait |
| `drr` | 59.8k | **60.6k** | `conn_data_waits=50`, the allocator did not wait |

The hog runs into its 32 MiB (`max_data_per_conn`), holds 32 buffers out of 64 and is no longer
read from the socket; the victim is left 24 units of the budget, enough for depth 16. After 30 s
the write timeout would close the hog altogether. Closes §4.6 of the `in-flight-limit` report.

**The allocator as the last line of defence.** In every run with the budget `allocator_waits` is
0–69 per hundreds of thousands of requests, with a total wait of 0.1–6 ms: that is the first poll
of `acquire_many` returning `Pending` during a concurrent buffer return, not an exhaustion of
the pool. Without the budget the same loads give seconds of waiting in the allocator (E4 read:4k:
18.7 s, E6: 33 s).

## 4. Fairness

### E7 and E7b. A heavy client (depth 2048) and a light one (depth 1), backend 1 ms, 64 workers

| Server | Heavy, ops/s | Light: ops/s, p50 / p99 |
|---|---|---|
| `main` | 56.8k | 28, **34.8 / 42.0 ms** |
| `inflight` 256 | 57.1k | 211, 4.5 / 7.9 ms |
| `inflight` 64 | 46.2k (−19 %) | 683, 1.3 / 3.2 ms |
| `budgets` | 57.3k | 218, 4.4 / 7.6 ms |
| `drr`, quantum 32 | 57.0k | 534, 1.8 / 3.8 ms |
| `drr`, quantum 8 | 57.0k | 636, **1.4 / 3.4 ms** |
| `drr`, quantum 1 | 56.6k | 647, 1.3 / 3.8 ms |

The latency of the light client with DRR is the wait for a free worker (all 64 are busy with
the heavy one and free up at 64 per ms) plus the remainder of the heavy flow's quantum: with a
quantum of 32 that is up to 32 dispatches, about 0.5 ms, with a quantum of 8 about 0.1 ms. A
quantum of 8 gives the isolation of limit 64 without its price of 19 % of throughput; a quantum
of 1 no longer improves the p50 but makes 790k rotations instead of 100k. The default in the
code is changed to 8.

### E8. 256 and 1024 active connections (depth 16 each) and a probe of depth 1, backend 1 ms

| Connections | Server | Total throughput | Probe p50 / p99 | `pending_waits` |
|---|---|---|---|---|
| 256 | `main`, `inflight` | 57.8k ops/s | **69.6 / 75.8 ms** | |
| 256 | `budgets` | 57.4k | **9.0 / 12.3 ms** | 807k |
| 256 | `drr` | 58.1k | 11.0 / 13.8 ms | 815k |
| 1024 | `main`, `inflight` | 58.1k | **278.5 / 301 ms** | |
| 1024 | `budgets` | 58.0k | **22.0 / 26.1 ms** | 829k |
| 1024 | `drr` | 58.4k | 21.5 / 26.6 ms | 834k |

The total demand (4096 and 16 384 in flight) is 64–256 times what the backend serves in 1 ms,
so a queue is unavoidable. Without `max_pending` it lies entirely in the server and the probe
waits for all of it. With `max_pending = 256` the server holds 256 requests, the rest is in the
TCP buffers; the probe waits one round-robin pass over the waiting connections (≈ N / 58k) plus
256 / 58k: the estimate gives 8.8 ms for 256 and 22 ms for 1024 connections, measured 9.0 and
22.0. The throughput does not change. With equal demand DRR adds nothing: the FIFO of the
waiters on `max_pending` already goes round the connections.

### E9. A fast client (depth 256) and a slow one (depth 2), backend 1 ms

| Server | Fast, ops/s | Slow: ops/s, p50 / p99 |
|---|---|---|
| `main` | 54.5k | 436, 4.5 / 6.9 ms |
| `inflight` | 57.4k | 455, 4.2 / 6.1 ms |
| `budgets` | 57.6k | 453, 4.2 / 6.3 ms |
| `drr` | 56.8k | **1131, 1.7 / 3.4 ms** |

No variant splits the resource equally: the fast one gets everything the slow one does not ask
for. With DRR the requests of the slow one do not wait behind the 256 requests of the fast one:
its throughput and latency improve 2.5 times, the fast one loses ~1 % (within noise).

### E10 and E10b. Metadata next to a stream of READ 1 MiB, device 2 GiB/s, latency 0.5 ms

| Configuration | Server | Stream, MiB/s | Metadata: ops/s, p50 / p99 |
|---|---|---|---|
| stream d32, 64 workers | `main` | 2039 | 40.7k, 1.38 / 4.99 ms |
| | `inflight`, `budgets` | 2044 | 41.6k, 1.34 / 4.35 ms |
| | `drr` | 2043 | 44.3k, 1.31 / 3.52 ms |
| stream d256, 32 workers | `main` | 2044 | 576, **110.6 / 118.8 ms** |
| | `inflight` | 2046 | 586, 108.5 / 118.8 ms |
| | `budgets` | 2048 | 790, 79.9 / 88.1 ms |
| | `drr` | 2048 | **4263, 15.9 / 30.2 ms** |

While the queue of the stream is not deeper than the pool (d32 on 64 workers) FIFO copes: 32
workers are free for metadata. When the queue of the stream is deep (d256 on 32 workers) FIFO
puts every GETATTR behind hundreds of 1 MiB reads; DRR by cost (GETATTR = 1, READ 1 MiB = 17
units) gives the metadata seven times more requests and seven times lower latency, and the
stream stays at the device ceiling.

## 5. What this means for the design

1. **PR #450 is confirmed**: the memory and isolation figures reproduce on the current `main`.
2. **The global budgets (phase 1) can be enabled by default.** `max_pending` and the byte budget
   cost nothing measurable, close the allocator starvation and bound the latency with thousands
   of connections; `max_connections` is trivial. The write timeout works, but its implementation
   through `tokio::time::timeout` per reply costs ~3 % on a saturated CPU: a per-connection
   watchdog is needed.
3. **DRR (phase 2) not only gives QoS but is cheaper than the current pool**: +4…+6 % on four
   connections, +30 % at depth 1. The default quantum should be 8. The flow key is the
   connection for now; moving to the client waits for `ClientId` (#327/#419).
4. **The allocator stays the floor** and does not wait in the normal mode: `allocator_waits` ≈ 0
   in every run with the budget.
5. **Not measured**: several backends of different speed, NLM/MOUNT under the same permits, the
   kernel NFS client, a network instead of loopback, a pool with size classes (a READ of 4 KiB
   still takes 1 MiB).

## 6. Limitations of the stand

A VM with ±5–15 % noise (large differences are confirmed by the counters), loopback, a mock
backend with an exact latency, two server worker threads, a load that is mostly GETATTR, one
load process per role. The absolute ops/s of the stand do not transfer to hardware; the relative
conclusions and the counters do.

## Appendix: reproduction

```text
research/qos-limits/
├── README.md          this report
├── bench/bench.py     the runner: e1 e2 e4 e5 e6 e7 e8 e9 e10 e2b e7b e10b
├── bench/summarize.py results.jsonl → markdown
├── bench/bin/         the binaries (not in git): server-main, server-exp, nfs-bench, slow-reader
└── data/              results.jsonl, run.log, summary.md
```

```bash
# the binaries of the current branch
cargo build --release -p mock-vfs
cp target/release/mock-nfs-server research/qos-limits/bench/bin/server-exp
cp target/release/nfs-bench target/release/slow-reader research/qos-limits/bench/bin/
```

```bash
# the baseline: a worktree of main with the same mock-vfs crate
git worktree add /tmp/wt-main 70d688a
cp -r mock-vfs Cargo.toml /tmp/wt-main/
(cd /tmp/wt-main && cargo build --release -p mock-vfs)
cp /tmp/wt-main/target/release/mock-nfs-server research/qos-limits/bench/bin/server-main
```

```bash
cd research/qos-limits/bench && python3 -I bench.py && python3 -I summarize.py > ../data/summary.md
```
