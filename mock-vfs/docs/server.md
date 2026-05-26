# mock-nfs-server

Standalone NFSv3 server with an in-memory mock filesystem.  
Used as the target for remote benchmarks.

## Build

```bash
cargo build --release -p mock-vfs --bin server
```

## Usage

```bash
cargo run --release -p mock-vfs --bin server [OPTIONS]
```

### Options

| Arg | Default | Description |
|---|---|---|
| `--bind <addr>` / `-b <addr>` | `0.0.0.0:2049` | Address to bind to |

### Examples

```bash
# default — listen on 0.0.0.0:2049
cargo run --release -p mock-vfs --bin server

# custom port
cargo run --release -p mock-vfs --bin server -- --bind 0.0.0.0:2049

# loopback only
cargo run --release -p mock-vfs --bin server -- --bind 127.0.0.1:2049

# random port (useful for local testing)
cargo run --release -p mock-vfs --bin server -- --bind 127.0.0.1:0
```

### Firewall

For remote access, ensure port 2049 is open:

```bash
# ufw
ufw allow 2049

# iptables
iptables -A INPUT -p tcp --dport 2049 -j ACCEPT
```
