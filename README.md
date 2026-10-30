# NFS Mamont

An asynchronous library to provide access to NFS protocol.

NFS Mamont is a from-scratch, user-space implementation of the NFS protocol
family. It is built on `tokio` and uses its own RPC/XDR encoder and decoder,
providing a complete MOUNT and NFSv3 stack. The project currently targets
**NFSv3** support, with NFSv4 planned as a future milestone.
> [!CAUTION]
> Project is  under active development now

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

As an example we provide our [demo](https://github.com/RMamonts/mirror-fs) implementation

The library does not install a global `tracing` subscriber — that is the responsibility
of the application. In your `main()`, initialize `tracing` yourself, for example:

```rust
tracing_subscriber::fmt()
    .with_env_filter(
        tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("nfs_mamont=debug")),
    )
    .init();
```

This requires `tracing-subscriber` (with the `env-filter` feature) as a dependency of your
application, not of the library.

## Safety & Requirements

- **Rust version:** 1.75.0 or later (CI is validated on both `1.75.0` and the
  current stable toolchain).
- **Platform:** Linux (`libc` is an optional dependency used only for the
  `mlock` feature, only needed if you are using our allocator implementation).

### Optional feature

- `mlock` — enables the `libc`-based memory locking helpers.

## Contributing

Contributions are welcome! See our [CONTRIBUTING](CONTRIBUTING.md) guide for
detailed instructions on reporting issues, setting up the environment, code
style, and the pull request workflow. Check the issues in the TODO column on
our [Kanban board](https://github.com/orgs/RMamonts/projects/1/views/1) for
tasks that are ready to be picked up.

## Roadmap

- [x] NFSv3 protocol
  - [x] MOUNT protocol
  - [ ] NSM protocol
  - [ ] NLMv4 file locking
- [ ] NFSv4 protocol

## License

This project is licensed under the MIT License — see the [LICENSE](LICENSE)
file for details.

claude test
