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
- **kani** — formal verification of the XDR padding and NLM interval
  arithmetic with [Kani](https://model-checking.github.io/kani/) (separate
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

A few pieces of arithmetic that untrusted network input reaches directly are
model-checked with Kani. Proof harnesses live next to the code they prove, in
`#[cfg(kani)] mod verification` blocks, and are invisible to a normal build.
They currently cover:

- XDR alignment padding on both the parser and serializer side: it consumes
  (emits) fewer than `ALIGNMENT` bytes for every length, keeping its slice index
  in bounds, and the count it picks really does restore alignment. The formula is
  duplicated in the parser and the serializer, and proving both independently is
  what keeps the two copies honest;
- the NLM interval arithmetic behind byte-range locks: an interval's end never
  precedes its start, length and end round-trip through each other, and
  `ranges_overlap` is reflexive, symmetric, and true exactly when the two ranges
  share a byte. `find_conflict` compares a stored lock against a request in one
  order only, so an asymmetric or backwards answer would hand two clients an
  exclusive lock over the same byte --- from nothing worse than the `offset` and
  `length` a client puts in a `LOCK` request.

Both groups are proved over the whole `usize` / `u64` domain rather than up to a
bound, which is what makes them cheap enough to keep in CI.

Kani complements `cargo-fuzz` rather than replacing it: fuzzing explores the
real async stack in depth, Kani proves these cores exhaustively. The async
layers (`tokio`) are out of scope for these harnesses and stay covered by tests
and fuzzing.

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