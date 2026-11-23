# mock-vfs

Benchmark tooling for NFS Mamont: an in-memory mock `Vfs` backend with a
configurable response time, a standalone server built on it, and a load
generator that speaks NFSv3 over TCP.

The point of the crate is to measure the **server** (RPC parsing, buffer
allocation, serialization, TCP handling, the VFS worker pool) against a backend
whose behaviour is known exactly, instead of against a disk or a remote storage
whose own latency dominates the numbers. The crate is a workspace member so CI
keeps it compiling, but it is never published.

```text
mock-vfs/
├── src/config.rs     MockVfsConfig and Latency (the backend model)
├── src/latency.rs    sub-millisecond timer + delay computation
├── src/tree.rs       the deterministic virtual directory tree
├── src/mock/         MockVfs: all 21 NFSv3 procedures
├── src/server.rs     wires the mock into nfs_mamont::handle_forever
├── src/client/       minimal NFSv3/MOUNT client (framing, XDR, requests)
├── src/hist.rs       latency histogram
├── src/cli.rs        flag parsing shared by the binaries
├── src/bin/mock-nfs-server.rs
├── src/bin/nfs-bench.rs
└── tests/e2e.rs      server + client over a real socket
```

## Quick start

```bash
# Everything in one process: an embedded mock server with a 200 µs backend,
# 4 connections keeping 16 requests in flight each.
cargo run --release -p mock-vfs --bin nfs-bench -- \
    --latency 200us --connections 4 --depth 16 \
    --op getattr,read:4k,read:64k,write:64k,randrw:4k --duration 10s
```

```text
op                  ops/s     MiB/s       avg       p50       p99     p99.9       max  errors
getattr             92371       0.0     266us     260us     396us     519us    4.13ms       0
read:4K             88156     344.4     295us     279us     460us     640us    4.57ms       0
...
```

Two machines: start the server on one, point the benchmark at it from the other.

```bash
# machine A
cargo run --release -p mock-vfs --bin mock-nfs-server -- --bind 0.0.0.0:2049 --latency 1ms

# machine B
cargo run --release -p mock-vfs --bin nfs-bench -- --target 192.168.1.100:2049 \
    --connections 8 --depth 32 --op read:1m,write:1m --duration 30s
```

The server can also be mounted with the Linux kernel client, which is handy for
looking at the server from the outside (`ls`, `dd`, `fio` on the mount point):

```bash
mount -t nfs -o vers=3,proto=tcp,port=2049,mountport=2049,nolock 127.0.0.1:/ /mnt
```

`nfs-bench` works against any NFSv3 server, not only the mock: it obtains the
root handle with a real `MNT` call and the file handle with `LOOKUP`.

## The backend model

Every procedure of the mock first waits for the configured latency; READ and
WRITE additionally pay for the bytes they move. A request transferring `bytes`
bytes issued at `now` completes at

```text
max(now, device_free_at) + bytes / bandwidth + base ± jitter
```

| Flag          | Meaning                                                                                   |
| ------------- | ----------------------------------------------------------------------------------------- |
| `--latency`   | `base`: fixed response time of the backend, paid by every request independently          |
| `--jitter`    | the delay is drawn uniformly from `[base - jitter, base + jitter]`                        |
| `--bandwidth` | throughput of one simulated device shared by all READ/WRITE requests (`device_free_at`)  |

Two things follow from the model and are worth keeping in mind when reading
results:

- **Queue depth.** The server runs at most `--concurrency` procedures against
  the backend at the same time (the `vfs_concurrency` of `ServerContext::new`).
  With latency `L` the server cannot complete more than `concurrency / L`
  procedures per second, whatever the client does: 64 workers and 1 ms give
  64 000 ops/s. Raise `--concurrency` to model a backend with a deeper queue.
- **Timer precision.** `tokio::time::sleep` has millisecond granularity, which
  is useless for simulating a 100 µs SSD. The mock uses its own timer thread
  (`latency.rs`) that waits on a condition variable until the earliest deadline;
  on Linux it also lowers its timer slack, so the wake-up error is a few
  microseconds instead of ~50 µs. There is no busy-waiting: `--latency 0` adds no
  code path at all, and a non-zero latency costs one mutex and one heap push per
  request.

## The virtual tree

The mock keeps no state. Every answer is a pure function of the request, so
nothing is locked or allocated per request and the mock scales with the number
of VFS workers. The rules:

- Every directory lists `.`, `..`, `dir_0 … dir_{N-1}` and `file_0 … file_{M-1}`
  (`--dirs`, `--files`). The tree is infinite: `dir_3/dir_3/dir_3` exists.
- `LOOKUP` of **any** name succeeds. Names starting with `dir` are directories,
  every other name is a regular file of `--file-size` bytes.
- Handles are derived from the parent handle and the name (`Handle::new` with the
  backend id taken from the request), so they are deterministic, survive server
  restarts and never become stale. `fileid`s are derived the same way.
- READ returns whatever the buffer contains unless `--fill` is given, because
  touching the data is the one cost a real backend would not spend in the
  server's address space. WRITE drops the data and echoes the requested
  stability level, so clients follow their normal `UNSTABLE` + `COMMIT` pattern.
- `CREATE`/`MKDIR`/`REMOVE`/`RMDIR`/`RENAME`/`SETATTR` succeed when they are
  consistent with the naming rule (`mkdir foo` fails with `EXIST` because `foo`
  is a file; `CREATE` in `GUARDED`/`EXCLUSIVE` mode fails with `EXIST` because
  every name exists) and are then forgotten.
- Hard links, symbolic links and special files are not supported; `READDIR`
  honours cookies and the reply size limits of the client.

## nfs-bench

Each `--op` entry is one phase; phases run one after another with
`--connections` TCP connections keeping `--depth` requests in flight each.
Requests are pipelined and matched to replies by transaction id, so out-of-order
completion is handled. Only replies with `accept_stat == SUCCESS` and
`nfsstat3 == NFS3_OK` count as operations and bytes; everything else is an error
(the first one is printed).

| Column          | Meaning                                                  |
| --------------- | -------------------------------------------------------- |
| `ops/s`         | successful operations per second over the measured part  |
| `MiB/s`         | bytes actually confirmed by READ/WRITE replies           |
| `avg … max`     | client-side round trip, resolution about 3 %             |
| `errors`        | replies that were not `NFS3_OK`                          |

Operations: `read`, `write`, `randrw` (`--rwmix` percent of reads), `getattr`,
`lookup`, `access`, `readdir`, `readdirplus`, `create`, `remove`, `commit`,
`null`. Data operations take an optional block size: `read:1m`.
`--duration 0` runs until Ctrl+C, `--warmup` excludes the start of every phase
from the statistics, `--seed` makes the offset sequence reproducible.

Without `--target` the server flags below configure the embedded server.

## mock-nfs-server

| Flag                  | Default        | Meaning                                                   |
| --------------------- | -------------- | --------------------------------------------------------- |
| `--bind <addr>`       | `0.0.0.0:2049` | listening address                                         |
| `--export <path>`     | `/`            | the only path the MOUNT service accepts                   |
| `--latency <dur>`     | `0`            | backend response time (`200us`, `1.5ms`, `1s`)            |
| `--jitter <dur>`      | `0`            | half-width of the uniform jitter                          |
| `--bandwidth <rate>`  | unlimited      | device throughput shared by READ/WRITE (`2G` = 2 GiB/s)   |
| `--concurrency <n>`   | `64`           | VFS workers, i.e. backend queue depth                     |
| `--buf-size <bytes>`  | `1M`           | size of one allocator buffer                              |
| `--buf-count <n>`     | `64`           | number of allocator buffers                               |
| `--max-io <bytes>`    | `1M`           | `rtmax`/`wtmax` advertised through FSINFO                 |
| `--file-size <bytes>` | `1G`           | size of every regular file                                |
| `--files <n>`         | `128`          | regular files listed in every directory                   |
| `--dirs <n>`          | `8`            | subdirectories listed in every directory                  |
| `--fill <byte>`       | none           | fill READ buffers with this byte                          |

Logging follows `RUST_LOG` and defaults to `warn`, so the per-request debug logs
of the library do not distort measurements. Like every NFS Mamont server, the
binary (and the embedded server of `nfs-bench`) tries to register its port with
a local `rpcbind`; without one, clients need explicit `port=`/`mountport=`.

## Tests

```bash
cargo test -p mock-vfs
```

Unit tests cover the tree, the latency model, the timer, the histogram, the
flag parsing and every procedure of the mock; `tests/e2e.rs` starts a real
server on a loopback port and drives it with the client of this crate.
