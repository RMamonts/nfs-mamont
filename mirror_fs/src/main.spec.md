<!-- SPEC_HASH: 384cb35624376cc971f10f77f1ea6023061f1b081637585107ae2c2a34dcb350 -->
# Module Specification

Module: mirrorfs
Rust File: src/main.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`clap::Parser`**: Used to parse command-line arguments provided by the user at runtime. It transforms raw argument strings into the structured `args::Args` type, handling validation and default values (e.g., the bind address).
- **`tokio::net::TcpListener`**: Used to bind to the network address specified in the arguments. It provides the stream of incoming TCP connections that the NFS server will process.
- **`nfs_mamont::handle_forever`**: Used as the primary runtime loop for the server. It takes ownership of the initialized listener, context, and services, managing the asynchronous task lifecycle for accepting connections and dispatching RPC requests.
- **`nfs_mamont::ServerContext`**: Used as a container to aggregate and pass shared resources (the VFS backend, memory allocators, and VFS worker pool) to the connection handlers spawned by `handle_forever`.
- **`nfs_mamont::Impl`**: Used as the concrete implementation of the memory allocator. Two instances are created: one for read buffers and one for write buffers, configured with sizes and counts from the loaded configuration.
- **`nfs_mamont::service::mount::MountService`**: Used to implement the MOUNT protocol. It is initialized with the list of exports resolved from the configuration file to handle client mount requests.
- **`nfs_mamont::service::nlm::NlmService`**: Used to implement the Network Lock Manager (NLM) protocol. It is initialized to handle file locking requests from clients.
- **`nfs_mamont::vfs::file::Path`**: Used to represent the directory paths in the export entries. The module converts local filesystem paths from the configuration into this VFS-compliant type.
- **`mirrorfs::args::Args`**: Used to hold the parsed command-line configuration (config file path and bind address).
- **`mirrorfs::config::load_config`**: Used to read and parse the TOML configuration file. It provides the validated settings for allocator sizes, VFS pool size, export root, and the list of exports.
- **`mirrorfs::fs::MirrorFS`**: Used as the concrete implementation of the Virtual File System (VFS). It bridges the gap between the generic NFS protocol requirements and the actual local filesystem operations.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

### Mechanism 1: Application Bootstrap and Composition (`main`)

**Intent:**
To serve as the "composition root" of the application. This mechanism is responsible for initializing all subsystems (logging, configuration, memory allocators, VFS, network) and wiring them together before starting the server loop. It translates high-level user configuration into the specific concrete types required by the `nfs_mamont` library.

**Inputs:**
- **Command-line arguments**: Implicitly read via `clap` (config path, bind address).
- **Configuration file**: Read from the path specified in arguments (TOML format).
- **Filesystem state**: Accessed via `MirrorFS` to resolve export handles.

**Outputs:**
- `std::io::Result<()>`: Indicates success or failure during the initialization phase. If successful, the function runs indefinitely (until the server is stopped or an error occurs in the loop).

**Steps:**
1. **Logging Initialization**: Calls `init_tracing()` if compiled in debug mode to set up structured logging.
2. **Argument Parsing**: Uses `Args::parse()` to process CLI arguments.
3. **Configuration Loading**: Calls `config::load_config(&args.config_path)` to read and validate the TOML configuration.
4. **VFS Instantiation**: Creates `MirrorFS::new(config.export_root.clone())`, wrapping it in an `Arc` for shared ownership.
5. **Allocator Instantiation**: Creates two `Impl` allocators (read and write) using buffer sizes and counts from the configuration.
6. **Context Creation**: Instantiates `ServerContext` with the VFS backend, allocators, and VFS pool size from the config.
7. **Network Binding**: Binds a `TcpListener` to the address specified in `args.addr`.
8. **Export Resolution**:
 - Iterates through the `exports` list defined in the configuration.
 - For each export, calls `fs.handle_for_path(&export.local_path).await` to resolve the local filesystem path to a VFS `Handle`.
 - If resolution fails, maps the error to `std::io::ErrorKind::InvalidInput` and returns early.
 - Constructs an `ExportEntryWrapper` containing the VFS path and the resolved handle.
9. **Service Instantiation**:
 - Creates `MountService::with_exports(exports)` using the resolved export list.
 - Creates `NlmService::new()`.
10. **Server Start**: Calls `handle_forever(listener, context, mount_service, nlm_service).await` to begin accepting and processing connections.

**Edge Cases:**
- **Export Resolution Failure**: If an export path defined in the configuration does not exist or cannot be accessed, the application logs an error and terminates before starting the listener.
- **Invalid Configuration**: If the TOML configuration is missing required fields or contains invalid values (e.g., zero buffer sizes), `load_config` will return an error, terminating the application.

**Complexity:**
- **Time**: O(N) for export resolution, where N is the number of exports. Other steps are O(1).
- **Space**: O(1) additional space on top of the resources allocated by the dependencies.

**Determinism:**
- **Deterministic**. Given the same configuration file and filesystem state, the initialization sequence and resulting server state are identical.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::lib`**:
 - **`handle_forever`**: This module relies on this function to manage the runtime. It passes the fully initialized `ServerContext`, `MountService`, and `NlmService` to it, delegating the entire connection handling and RPC dispatching logic to the library.
 - **`init_tracing`**: Used to set up global logging infrastructure in debug builds.

- **From `nfs_mamont::context`**:
 - **`ServerContext::new`**: This module uses the constructor to aggregate the VFS backend and allocators. It relies on the context to manage the lifecycle of the `VfsPool` and provide accessors for the connection handlers.

- **From `nfs_mamont::allocator`**:
 - **`Impl::new`**: This module uses this to create the memory pools. It passes the specific buffer sizes and counts from the configuration to enforce memory limits defined by the user.

- **From `nfs_mamont::service::mount`**:
 - **`MountService::with_exports`**: This module uses this to initialize the MOUNT protocol handler. It performs the critical step of resolving local paths to VFS handles *before* calling this constructor, ensuring the service starts with a valid export table.

- **From `mirrorfs::fs`**:
 - **`MirrorFS::new`**: This module instantiates the specific VFS implementation. It passes the `export_root` from the configuration, defining the scope of the filesystem that the server will expose.

- **From `mirrorfs::config`**:
 - **`load_config`**: This module uses this to translate the static TOML file into runtime configuration structs (`Config`, `AllocatorConfig`, `ExportConfig`), which are then used to parameterize the allocators and VFS.

---

## 4. Data Model

Entities:
- This module defines no public structs or enums. It acts as a coordinator using types defined in its dependencies.

Relations:
- **`main` → `ServerContext` (Ownership)**: Creates and owns the context, passing it to `handle_forever`.
- **`main` → `MountService` (Ownership)**: Creates and owns the service, passing it to `handle_forever`.
- **`main` → `NlmService` (Ownership)**: Creates and owns the service, passing it to `handle_forever`.
- **`Config` → `ExportEntryWrapper` (Transformation)**: The module transforms `ExportConfig` from the config file into `ExportEntryWrapper` by resolving the `local_path` to a `Handle`.

Global Invariants:
- **Export Validity**: The `MountService` is guaranteed to be initialized only with exports that have been successfully resolved to valid VFS handles. If any export fails resolution, the server does not start.

## 5. Error Model

Error Types:
- **`std::io::Error`**: The primary error type returned by the `main` function.

Error Propagation Strategy:
- **Early Termination**: The `main` function uses the `?` operator to propagate errors immediately. If configuration loading, listener binding, or export resolution fails, the error is returned, causing the application to exit.

Recoverability:
- **Non-recoverable**. Since this is the entry point, any error during the initialization phase is considered fatal, and the process terminates.

Panics:
- **Allowed**: Indirectly.
- **Conditions**:
 - If `clap` fails to parse arguments (it usually prints help and exits, but can panic in internal error cases).
 - If `tokio::spawn` or `handle_forever` encounters a runtime failure (e.g., executor shutdown).

---

## 6. Traits

List which external traits this module implements:
- None.

---

## 7. Overview

This module is used in order to **bootstrap and configure the `mirrorfs` NFS server**, acting as the bridge between user-defined configuration and the generic `nfs_mamont` server framework. It is necessary because the `nfs_mamont` library is a generic engine that does not know *what* filesystem to serve or *how* much memory to use; this module provides those specific details.

The system contains a high-performance, user-space NFSv3 server implementation (`nfs_mamont`) and a specific filesystem backend that mirrors a local directory (`mirrorfs::fs`). This module is the "glue" that binds them together. It reads the user's intent (via CLI args and a TOML file) and constructs the necessary objects: it sets up the memory pools with the requested sizes, initializes the VFS with the requested root directory, and resolves the specific exports to their internal file handles.

A typical usage scenario of the system involves a system administrator starting the binary: `./mirrorfs -c /etc/nfs.toml`. The `main` function parses this, reads the TOML to find out which directories to export (e.g., `/mnt/storage`), and initializes the `MirrorFS` to serve that directory. It then allocates memory buffers for network I/O and starts the `TcpListener`. Finally, it hands control over to `nfs_mamont::handle_forever`, which runs the server loop.

Inside the system, the following things happen and they use this module:
1. **Dependency Injection**: The module constructs the `ServerContext`, injecting the specific `MirrorFS` implementation and the configured `Impl` allocators into the generic server framework. This allows the framework to handle NFS protocols without knowing the details of the underlying storage or memory management.
2. **Export Resolution**: Before the server starts, this module iterates over the exports defined in the config and calls `fs.handle_for_path`. This proactive resolution ensures that the paths are valid and that the file handles are ready. It converts the string-based paths from the config into the `ExportEntryWrapper` structures required by the `MountService`.
3. **Runtime Orchestration**: The module defines the startup sequence: logging first, then config, then VFS/Allocators, then network binding. This sequence ensures that resources are available and valid before the server attempts to accept connections.

The critical aspect of this module is the **translation of configuration into concrete types**. It validates the user's setup (e.g., "Do these export paths exist?") and converts abstract settings (e.g., "buffer size = 64kb") into the `Arc<Impl>` and `Arc<MirrorFS>` instances that drive the server.

**Uncertainty**: The module declares `pub mod fs_map;` but does not use it in the provided code. It is possible `fs_map` is intended for future use or is used by other parts of the `mirrorfs` crate not visible in this context. Additionally, the `fs_map` specification suggests it handles path-to-handle mapping, but `main.rs` calls `fs::MirrorFS::handle_for_path` directly. This implies `MirrorFS` might internally use `fs_map` or provide a similar interface, but the direct dependency link in `main.rs` is to `fs`, not `fs_map`.