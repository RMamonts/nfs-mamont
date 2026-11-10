# nfs-mamont-util

Ready-made building blocks for servers built on
[`nfs-mamont`](https://crates.io/crates/nfs-mamont).

This crate implements the traits declared in `nfs-mamont`. It uses only the public API of
`nfs-mamont`, so it also serves as a reference for writing your own implementations.

[API documentation](https://docs.rs/nfs-mamont-util) |
[Project overview](https://github.com/RMamonts/nfs-mamont#readme)

## Installation

```sh
cargo add nfs-mamont-util
```

## Contents

| Item | Role |
|------|------|
| `PoolAllocator` | `nfs_mamont::Allocator` backed by a pool of reusable byte buffers |
| `Slice` | Buffer handed out by `PoolAllocator` |
| `MountService` | MOUNT v3 service, an implementation of `nfs_mamont::mount::Mount` |
| `NlmService` | NLM v4 service, an implementation of `nfs_mamont::nlm::Nlm` |

## Feature flags

- `mlock` — locks the whole `PoolAllocator` memory block in RAM via `libc`. Disabled by default.

## License

This project is licensed under the [MIT License](LICENSE).
