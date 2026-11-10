# nfs-mamont

An asynchronous NFS server library for Rust.

`nfs-mamont` is a from-scratch, user-space implementation of the NFS protocol family, built on
`tokio` with a hand-written RPC/XDR parser and serializer. It provides the MOUNT and NFSv3 stack
and leaves storage to your code through a pluggable VFS trait.

> [!CAUTION]
> The project is under active development, and the API may change between releases.

[API documentation](https://docs.rs/nfs-mamont) |
[Project overview](https://github.com/RMamonts/nfs-mamont#readme) |
[Demo server](https://github.com/RMamonts/mirror-fs)

## Installation

```sh
cargo add nfs-mamont
```

## Usage

### Traits to implement

`nfs-mamont` handles the wire protocols and calls into your code through traits. A server needs
one implementation of each:

| Trait | What your implementation does | Ready-made implementation |
|-------|-------------------------------|---------------------------|
| `Vfs` | Serves NFSv3 procedures from your storage | None, this one is always yours |
| `Mount` | Handles MOUNT requests and exports | `MountService` |
| `Nlm` | Handles NLM file locking | `NlmService` |
| `Allocator` | Provides the buffers used for packet and file I/O | `PoolAllocator` |

The ready-made implementations are in
[`nfs-mamont-util`](https://crates.io/crates/nfs-mamont-util). `nfs-mamont` does not depend on it,
so you only need it if you do not write your own.

`Vfs`, `Mount` and `Nlm` have one sub-trait per protocol procedure, for example `vfs::read::Read`
or `mount::mnt::Mnt`. Implement the sub-traits: the umbrella trait then applies automatically.

### Running the server

Pass your implementations to these items, which `nfs-mamont` provides:

| Item | Role |
|------|------|
| `ServerContext` | Holds the allocator and the `Vfs` backends |
| `handle_forever` | Accepts and processes client connections on a `TcpListener` |

For a complete server, see the [demo](https://github.com/RMamonts/mirror-fs).

## Logging

The library reports through `tracing` and does not install a global subscriber: that is the
responsibility of the application. For example, in your `main()`:

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

## License

This project is licensed under the [MIT License](LICENSE).
