# NFS Mamont

An asynchronous **Network File System (NFS) server** build-in.

NFS Mamont is a from-scratch, user-space implementation of the NFS protocol
family. It is built on `tokio` and uses its own RPC/XDR encoder and decoder,
providing a complete MOUNT and NFSv3 stack. The project currently targets
**NFSv3** support, with NFSv4 planned as a future milestone.

## Features

- **NFSv3** via the `NFS_PROGRAM` (100003) RPC program — full procedure set of RFC 1813.
- **MOUNT protocol** — export handling, mount/umount, and export dumps.
- **Custom RPC/XDR layer** — hand-written parser and serializer.
- **Zero-copy buffer allocator** — a reusable byte-slice allocator for
  efficient packet and file I/O. Allocator has public trait so you can put your own implementation.
- **Pluggable VFS backend** — the `Vfs` trait abstracts the filesystem so
  different storage back ends can be mounted.

## Architecture

Described [here](https://github.com/RMamonts/nfs-mamont/wiki/NFS%E2%80%90Mamont-architecture)
### Quick start

As an example we provide our [demo](https://github.com/RMamonts/mirror-fs) implementaion

## Safety & Requirements

- **Rust version:** 1.75.0 or later (CI is validated on both `1.75.0` and the
  current stable toolchain).
- **Platform:** Linux (`libc` is an optional dependency used only for the
  `mlock` feature, only needed if you are using our allocator implementation).

## Development

The repository uses a lightweight build setup and a GitHub Actions CI pipeline
with the following jobs:

- **lint** — `rustfmt`, `clippy` (`-D warnings`), and `cargo check --locked`.
- **test** — build and run the test suite across MSRV (`1.75.0`) and stable
  (`1.95.0` - can be cahnged - look CI file `.github` folder).
- **security** — `cargo audit` for known vulnerabilities in dependencies.
- **docs** — build documentation with warnings-as-errors.
- **udeps** — detect unused dependencies via `cargo-udeps`.

### Local commands

```bash
cargo build                      # build the library
```

```bash
cargo test                       # run tests
```

```bash
cargo clippy -- -D warnings      # static analysis
```

```bash
cargo fmt --all --check          # formatting check
```

```bash
cargo doc --no-deps              # build docs
```

### Optional feature

- `mlock` — enables the `libc`-based memory locking helpers.

## Contributing

Contributions are welcome! Check the issues in the TODO column on our
[Kanban board](https://github.com/orgs/RMamonts/projects/1/views/1).

Please run the lint, test, and audit commands above before submitting a pull
request, and follow existing code style (`rustfmt`).

## Roadmap

- [x] NFSv3 protocol
  - [x] MOUNT protocol
  - [ ] NSM protocol
  - [ ] NLMv4 file locking
- [ ] NFSv4 protocol

## License

This project is licensed under the MIT License — see the [LICENSE](LICENSE)
file for details.