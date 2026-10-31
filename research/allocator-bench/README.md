# allocator-bench

Benchmarks and prototypes used for the allocator study
(see [`../ALLOCATOR_REPORT.md`](../ALLOCATOR_REPORT.md)).

This is a standalone crate (own `[workspace]`), it does not take part in the
`nfs-mamont` workspace build, CI, or `Cargo.lock`. It depends on the library through a
path dependency and only uses its public API (`Allocator`, `Buffer`, `Impl`, `Slice`,
`handle_forever`, ...).

## Contents

| path | what |
|---|---|
| `src/pool.rs` | generic async pool: one mutex, FIFO waiters, inline chunk list |
| `src/fixed.rs` | fixed-size blocks, LIFO free list (upstream semantics) |
| `src/lockfree.rs` | fixed-size blocks, lock-free LIFO stack, O(1) batch pop/push, atomic budget |
| `src/buddy.rs` | 4 KiB pages, buddy system, exact-size contiguous runs, optional hot caches |
| `src/arena.rs` | page/THP-aligned lazily zeroed `mmap` arena |
| `tests/pool.rs` | correctness: data integrity, waiting, FIFO, cancellation, multi-thread stress |
| `src/bin/micro.rs` | single-thread cost of `allocate`+drop, cost breakdown, alignment |
| `src/bin/contention.rs` | tokio multi-thread throughput (loop / pipeline / starved) |
| `src/bin/threads.rs` | same on plain OS threads (no scheduler noise) |
| `src/bin/locality.rs` | FIFO vs LIFO reuse: memcpy throughput, RSS growth, THP |
| `src/bin/hol.rs` | head-of-line blocking caused by one oversized request |
| `src/bin/e2e.rs` | real server (`handle_forever`) + mock VFS + pipelining READ/WRITE client |
| `scripts/*.sh` | drivers that produced `results/*.txt` |

## Running

```bash
cargo test --release
cargo run --release --bin micro
cargo run --release --bin threads
cargo run --release --bin contention
cargo run --release --bin hol
scripts/locality_matrix.sh > results/locality.txt
scripts/e2e_matrix.sh 3 2.5 > results/e2e_raw.txt
python3 scripts/e2e_table.py results/e2e_raw.txt
```

The prototypes are research code: they are tested, but are not meant to be merged
as is.
