# Pipeline Benchmark (latency)

Measures per-operation latency of individual NFS procedures (getattr, read, lookup, etc.)  
Uses [criterion](https://github.com/bheisler/criterion.rs) for statistical analysis.

## Build

```bash
cargo bench -p mock-vfs --bench pipeline -- --quick
```

## Usage

### Local mode (embedded server)

No setup needed — the benchmark starts its own server internally:

```bash
cargo bench -p mock-vfs --bench pipeline
```

### Remote mode (external server)

1. Start the server on the target machine (see [server.md](server.md))
2. Run the benchmark with `MOCK_TARGET`:

```bash
MOCK_TARGET=<ip>:<port> cargo bench -p mock-vfs --bench pipeline
```

Example:

```bash
# Machine A (server): starts listening
cargo run --release -p mock-vfs --bin server -- --bind 0.0.0.0:2049

# Machine B (client): benchmarks against A
MOCK_TARGET=192.168.1.100:2049 cargo bench -p mock-vfs --bench pipeline
```

### Criterion options

Criterion accepts standard flags, e.g.:

```bash
# Run only selected benchmarks
cargo bench -p mock-vfs --bench pipeline -- getattr

# Quick smoke test (fewer iterations)
cargo bench -p mock-vfs --bench pipeline -- --quick

# Save baseline for comparison
cargo bench -p mock-vfs --bench pipeline -- --save-baseline main

# Compare against baseline
cargo bench -p mock-vfs --bench pipeline -- --baseline main
```

## Measured procedures

| Name | NFS proc | Description |
|---|---|---|
| `getattr` | 1 | Get file attributes |
| `read_4k` | 6 | Read 4 KB |
| `read_64k` | 6 | Read 64 KB |
| `read_1m` | 6 | Read 1 MB |
| `write_64k` | 7 | Write 64 KB |
| `lookup` | 3 | Lookup filename in directory |
| `readdir` | 16 | List directory entries |
| `commit` | 21 | Commit pending writes |
