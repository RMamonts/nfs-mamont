<!-- SPEC_HASH: 384cb35624376cc971f10f77f1ea6023061f1b081637585107ae2c2a34dcb350 -->
# Module Specification

Module: mirrorfs
Rust File: src/main.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`mirrorfs::args`**: Used to parse command-line arguments. Specifically, it retrieves the path to the configuration file and the TCP address (`addr`) on which the server should listen.
- **`mirrorfs::config`**: Used to load and validate the server configuration from a TOML file. It provides the memory allocator settings (buffer sizes/counts), VFS pool size, and the list of filesystem exports (local paths and mount paths).
- **`mirrorfs::fs`**: Used to instantiate the `MirrorFS` backend. This module implements the VFS trait, bridging the local filesystem to the NFS protocol. It is used to resolve local export paths to NFS file handles.
- **`nfs_mamont::context`**: Used to create the `ServerContext`. This struct acts as a dependency injection container, holding the VFS backend, memory allocators, and the VFS worker pool configuration.
- **`nfs_mamont::allocator`**: Used to instantiate the memory allocators (`Impl`) for read and write operations. These allocators manage the fixed-size memory pools defined in the configuration.
- **`nfs_mamont::service::mount`**: Used to create the `MountService`. This service manages the state of exported directories and active client mounts. It is initialized with the list of exports resolved from the configuration.
- **`nfs_mamont::service::nlm`**: Used to create the `NlmService`. This service manages the Network Lock Manager state (file locks).
- **`nfs_mamont::vfs::file`**: Used to construct `VfsPath` objects. These represent the directory paths in the NFS protocol format and are used to define the exports in the `MountService`.
- **`nfs_mamont`**: Used to access the `handle_forever` function. This function starts the main server loop, accepting connections and dispatching them using the provided context and services.
- **`tokio::net::TcpListener`**: Used to bind to the network address specified in the command-line arguments and accept incoming TCP connections.
- **`clap::Parser`**: Used to derive the argument parsing logic for the `Args` struct in `mirrorfs::args`.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

### Mechanism 1: Server Bootstrap and Configuration (`main`)

**Intent:**
To initialize the entire NFS server stack. This involves loading configuration, setting up the filesystem backend, configuring memory allocators, resolving export handles, and starting the network listener.

**Inputs:**
- Command-line arguments (implicitly via `std::env::args`).
- Configuration file content (via `config::load_config`).

**Outputs:**
- `std::io::Result<()>`: Returns an I/O error if initialization fails, otherwise runs indefinitely.

**Steps:**
1. **Tracing Initialization**: Calls `init_tracing()` if compiled in debug mode to set up logging.
2. **Argument Parsing**: Parses command-line arguments using `args::Args::parse()` to get the config path and bind address.
3. **Configuration Loading**: Calls `config::load_config(&args.config_path)` to read and validate the TOML configuration.
4. **Backend Instantiation**: Creates an instance of `fs::MirrorFS` using the `export_root` defined in the configuration.
5. **Allocator Instantiation**: Creates two instances of `nfs_mamont::Impl` (the allocator implementation), one for read buffers and one for write buffers, configured with sizes and counts from the config.
6. **Context Creation**: Instantiates `ServerContext` with the `MirrorFS` backend, the allocators, and the `vfs_pool_size`.
7. **Network Binding**: Binds a `TcpListener` to the address provided in the arguments.
8. **Export Resolution**:
    - Iterates over the `exports` list from the configuration.
    - For each export, calls `fs.handle_for_path(&export.local_path).await` to resolve the local filesystem path to a VFS `Handle`.
    - If resolution fails, maps the error to `std::io::ErrorKind::InvalidInput` and returns.
    - Constructs an `ExportEntryWrapper` containing the `VfsPath` (derived from `mount_path`) and the resolved `root_handle`.
9. **Service Initialization**:
    - Creates `MountService` using the list of resolved exports.
    - Creates `NlmService` (default state).
10. **Server Start**: Calls `handle_forever(listener, context, mount_service, nlm_service).await` to begin accepting and processing connections.

**Edge Cases:**
- **Export Resolution Failure**: If a path specified in the configuration does not exist or is inaccessible, `handle_for_path` fails, and the server terminates with an `InvalidInput` error before starting the listener.
- **Invalid Mount Path**: If `VfsPath::new` fails (e.g., path too long), the server terminates.

**Complexity:**
- **Time**: O(N) where N is the number of exports, due to the loop resolving handles.
- **Space**: O(N) for storing the vector of exports.

**Determinism:**
- **Deterministic**: The startup sequence is linear and depends entirely on the configuration file and filesystem state.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `mirrorfs::config`**:
    - **`load_config`**: The module relies on this to transform the raw TOML file into a structured `Config`. It specifically depends on the validation logic within `load_config` to ensure that `export_root` exists and that `exports` are non-overlapping, preventing runtime errors later in the export resolution step.

- **From `mirrorfs::fs`**:
    - **`MirrorFS::new`**: The module uses this to create the concrete implementation of the VFS. It assumes `MirrorFS` implements the `nfs_mamont::vfs::Vfs` trait.
    - **`handle_for_path`**: The module uses this to perform the critical translation from local OS paths (strings) to NFS file handles (opaque bytes). This is necessary because the `MountService` requires handles, not paths, to identify exports.

- **From `nfs_mamont::context`**:
    - **`ServerContext::new`**: The module uses this to aggregate the backend and allocators into a single object. It relies on the context to manage the lifecycle of the `VfsPool` and provide access to the allocators for the connection handlers.

- **From `nfs_mamont::service::mount`**:
    - **`MountService::with_exports`**: The module uses this to initialize the MOUNT protocol state. It passes the list of `ExportEntryWrapper` (containing resolved handles) to this function. The module relies on the service to manage the mapping between these handles and client mount requests.

- **From `nfs_mamont::allocator`**:
    - **`Impl::new`**: The module uses this to create the memory pools. It relies on the allocator to enforce the buffer size and count limits defined in the configuration, ensuring the server does not exceed its allocated memory.

---

## 4. Data Model

Entities:
- This module defines no public structs or enums. It acts as the composition root for the application.

Relations:
- **`main` → `Config` (Usage)**: Reads configuration to drive initialization.
- **`main` → `MirrorFS` (Ownership)**: Creates and owns the backend instance (wrapped in `Arc`).
- **`main` → `ServerContext` (Ownership)**: Creates the context holding the backend and allocators.
- **`main` → `MountService` (Ownership)**: Creates the mount service with resolved exports.
- **`main` → `NlmService` (Ownership)**: Creates the lock manager service.

Global Invariants:
- **Export Validity**: The server guarantees that all exports registered with `MountService` correspond to valid, resolvable file handles in the `MirrorFS` backend at the moment of startup. If any export is invalid, the server does not start.

## 5. Error Model

Error Types:
- **`std::io::Error`**: The primary error type returned by `main`.

Error Propagation Strategy:
- **Early Exit**: The `main` function uses the `?` operator to propagate errors immediately. If configuration loading, argument parsing, listener binding, or export resolution fails, the error is returned up the stack, resulting in process termination.

Recoverability:
- **Non-recoverable**: The module does not implement retry logic or fallback mechanisms. If initialization fails, the application exits.

Panics:
- **Allowed**: Indirectly.
- **Conditions**:
    - If `VfsPath::new` panics (unlikely, it returns `Result`).
    - If `handle_forever` panics (depends on runtime errors).
    - The code itself uses `?` for fallible operations, avoiding explicit panics in the startup sequence.

---

## 6. Traits

List which external traits this module implements:
- None.

---

## 7. Overview

This module is used in order to **bootstrap and configure the `mirrorfs` NFS server**, acting as the entry point that ties the specific `mirrorfs` backend implementation to the generic `nfs_mamont` server framework.

The system contains a high-performance, user-space NFSv3 server implementation (`nfs_mamont`) and a specific backend that mirrors a local directory structure (`mirrorfs`). This module is necessary because the generic server framework is agnostic to *what* it serves and *how* it is configured. It requires a concrete implementation of the VFS trait, specific memory allocator parameters, and a list of exports to serve. This module provides that concrete implementation and configuration, effectively "wiring" the application together.

A typical usage scenario of the system involves an administrator starting the binary with a command like `./mirrorfs -c /etc/mirrorfs.toml`. The module parses the arguments, reads the TOML configuration to determine which directories to export and how much memory to use, and then initializes the `MirrorFS` backend to interface with the local filesystem. It resolves the configured export paths to internal file handles, ensuring they are valid before listening for connections. Finally, it starts the Tokio runtime and the `handle_forever` loop to accept NFS clients.

Inside the system, the following things happen and they use this module:
1.  **Dependency Injection**: The module acts as the "composition root." It instantiates the `MirrorFS` (the VFS implementation), the `Impl` allocators (the memory management strategy), and the `MountService`/`NlmService` (the protocol logic). It passes these instances to `ServerContext` and `handle_forever`. This ensures that the generic server logic has all the specific components it needs to run.
2.  **Export Resolution**: Before the server starts accepting connections, this module iterates through the exports defined in the configuration. It calls `handle_for_path` on the `MirrorFS` backend. This step is crucial because it validates that the exports exist and maps them to the opaque file handles required by the NFS MOUNT protocol. If this step fails, the server fails fast, preventing a half-started state.
3.  **Resource Configuration**: The module reads the `AllocatorConfig` from the TOML file and uses it to size the read and write memory pools. This allows the administrator to control the server's memory footprint via the configuration file, which this module enforces by passing the parameters to the allocator constructors.

Without this module, the `nfs_mamont` library would lack a specific backend to serve files, and the `mirrorfs` backend would lack a mechanism to start the network server. This module is the glue that transforms the library into a runnable application.

**Uncertainty**: The code imports `pub mod fs_map;` but does not explicitly use it in `main.rs`. It is assumed that `fs_map` is used internally by the `fs::MirrorFS` module (as suggested by the dependency specifications for `fs_map`), but `main.rs` interacts only with the public interface of `fs::MirrorFS`.