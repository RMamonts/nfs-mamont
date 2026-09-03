# NFS Mamont

An asynchronous library to provide access to NFS protocol.

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
- **kani** — formal verification of the parser, allocator and serializer
  cores with [Kani](https://model-checking.github.io/kani/) (separate
  workflow, runs on PRs that touch those paths).

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

```bash
cargo install --locked kani-verifier && cargo kani setup   # one-time
cargo kani -p nfs-mamont         # run proof harnesses
```

### Formal verification

The synchronous cores that parse untrusted network input are model-checked
with Kani. Proof harnesses live next to the code they prove, in
`#[cfg(kani)] mod verification` blocks, and are invisible to a normal build.
They currently cover:

- XDR alignment padding on both the parser and serializer side;
- the `ReadBuffer` position invariant `read_pos <= write_pos <= data.len()`,
  whose violation would be a panic reachable from the network --- proved for
  any operation sequence that respects the preconditions its setters do not
  check themselves; that `CountBuffer` respects them is not yet proved;
- the `Slice` iterators, which index buffers with a caller-supplied range.

Kani complements `cargo-fuzz` rather than replacing it: fuzzing explores the
real async stack in depth, Kani proves the synchronous cores exhaustively up
to a small bound. The async layers (`tokio`) are out of scope for these
harnesses and stay covered by tests and fuzzing.

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