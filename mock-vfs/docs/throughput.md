# Throughput Benchmark

Measures throughput (MB/s, ops/s) of concurrent NFS operations over a fixed duration.  
Unlike the pipeline benchmark, this stresses the server with parallelism and sustained load.

## Build

```bash
cargo build --release -p mock-vfs --bin throughput
```

## Usage

```bash
cargo run --release -p mock-vfs --bin throughput [OPTIONS]
```

### Options

| Arg | Env | Default | Description |
|---|---|---|---|
| `--target <addr>` | `MOCK_TARGET` | embedded server | NFS server address |
| `--mode <mode>` | — | `read` | `read`, `write`, or `randrw` |
| `--block-size <n>` | — | `65536` | Block size (supports K/M/G suffix) |
| `--connections <n>` | — | `4` | Number of concurrent connections |
| `--duration <secs>` | — | `5` | Test duration in seconds |
| `--rwmixread <%>` | — | `50` | Read percentage for randrw mode |

### Block size suffixes

- `4K` → 4096
- `64K` → 65536
- `1M` → 1048576
- `64M` → 67108864

### Modes

#### read

Each connection loops: NFS READ(offset, block_size) → wait response → repeat.  
Offset is random for each request.

```bash
cargo run --release -p mock-vfs --bin throughput -- --mode read --block-size 1M
```

#### write

Each connection loops: NFS WRITE(offset, block_size, data) → wait response → repeat.

```bash
cargo run --release -p mock-vfs --bin throughput -- --mode write --block-size 64K --connections 8
```

#### randrw

Each operation is randomly chosen as READ or WRITE based on `--rwmixread`.  
Offset is random for each request.

```bash
cargo run --release -p mock-vfs --bin throughput -- --mode randrw --block-size 4K --rwmixread 70
```

### Examples

```bash
# Local read throughput, 64 KB blocks, 4 connections, 5 seconds (defaults)
cargo run --release -p mock-vfs --bin throughput

# Local write, 1 MB blocks, 8 connections, 10 seconds
cargo run --release -p mock-vfs --bin throughput -- --mode write --block-size 1M --connections 8 --duration 10

# Remote randrw
cargo run --release -p mock-vfs --bin throughput -- --target 192.168.1.100:2049 --mode randrw

# Remote randrw with env var
MOCK_TARGET=192.168.1.100:2049 cargo run --release -p mock-vfs --bin throughput -- --mode randrw
```

## Output example

```
=== Throughput Results ===
Mode:        Randrw
Block size:  4096
Connections: 4
Duration:    1s

Reads:  93226 ops, 364.2 MB
Writes: 40052 ops, 156.5 MB

Total: 520.6 MB in 1s = 545.9 MB/s, 133278 ops/s
```
