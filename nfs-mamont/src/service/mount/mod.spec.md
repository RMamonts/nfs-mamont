<!-- SPEC_HASH: b6efefa10882c2b2bb1546eabf9fcdb901617426e21e8419f9fbda1f28f98d43 -->
# Module Specification

Module: nfs_mamont::service::mount
Rust File: src/service/mount/mod.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`std::collections::{HashMap, HashSet}`**: Used to implement the in-memory storage for both `ExportRegistry` and `MountRegistry`. `HashMap` provides O(1) lookups for exports by path and mounts by client address, while `HashSet` ensures that a single client does not have duplicate entries for the same directory.
- **`std::net::SocketAddr`**: Used as the key in `MountRegistry` to uniquely identify client connections for tracking active mounts.
- **`std::sync::Arc`**: Used to wrap `ExportRegistry` within `MountService`. This allows the export configuration (which is read-only after initialization) to be shared efficiently across asynchronous tasks without cloning the underlying data structure.
- **`tokio::sync::RwLock`**: Used to wrap `MountRegistry` within `MountService`. This provides thread-safe, asynchronous access to the dynamic mount state, allowing multiple concurrent reads (e.g., `DUMP` requests) or exclusive writes (e.g., `MNT`, `UMNT`) without blocking the runtime executor.
- **`crate::mount::{ExportEntry, MountEntry}`**: Used as the core data structures representing the configuration of an exported directory and the record of an active mount, respectively. These are stored within the registries.
- **`crate::rpc::AuthFlavor`**: Used to define the `AUTH` constant, which specifies the authentication flavors supported by the server (currently only `None`).
- **`crate::vfs::file`**: Used to import `file::Handle` and `file::Path`. `file::Handle` is stored in `ExportEntryWrapper` to represent the root file handle returned to clients, and `file::Path` is used as the key for looking up exports.
- **Sub-modules (`dump`, `export`, `mnt`, `umnt`, `umntall`)**: These modules contain the implementations of the specific MOUNT protocol procedures. They are declared here to organize the code and to allow them to access the private fields of `MountService` (since they are in the same module).

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

### Mechanism 1: Export Registry Initialization

**Intent:**
To construct a read-only index of exported directories that maps a filesystem path to its associated metadata and root file handle.

**Inputs:**
- `entries: Vec<ExportEntryWrapper>`: A list of export configurations, where each entry contains the user-visible `ExportEntry` and the VFS-generated `root_handle`.

**Outputs:**
- `ExportRegistry`: A populated registry struct.

**Steps:**
1. Create an empty `HashMap<file::Path, ExportEntryWrapper>`.
2. Iterate over the input `entries`.
3. For each entry, clone the `root_handle` (to ensure ownership within the map) and insert the `ExportEntryWrapper` into the map using `entry.export.directory` as the key.
4. Return the `ExportRegistry` wrapping the map.

**Edge Cases:**
- **Duplicate Paths**: If the input vector contains multiple entries for the same directory, the last one processed will overwrite the previous ones in the `HashMap`.

**Complexity:**
- **Time**: O(N), where N is the number of export entries.
- **Space**: O(N), to store the entries in the HashMap.

**Determinism:**
- **Deterministic**: The resulting map depends strictly on the order and content of the input vector.

### Mechanism 2: Mount Service Construction

**Intent:**
To instantiate the main service object, separating the static export configuration from the dynamic mount state.

**Inputs:**
- `entries: Vec<ExportEntryWrapper>`: The initial list of exports.

**Outputs:**
- `MountService`: A new service instance.

**Steps:**
1. Call `ExportRegistry::from_entries(entries)` to create the static index.
2. Wrap the `ExportRegistry` in an `Arc` to enable shared read-only access.
3. Create a default (empty) `MountRegistry`.
4. Wrap the `MountRegistry` in a `RwLock` to enable safe concurrent access.
5. Return the `MountService` struct containing these two fields.

**Complexity:**
- **Time**: O(N) due to the registry initialization.
- **Space**: O(N) for the exports + O(1) for the empty mount registry.

**Determinism:**
- **Deterministic**.

### Mechanism 3: Export Lookup

**Intent:**
To provide a synchronous (non-async) method for retrieving the metadata and file handle for a specific export path. This is used by the RPC handlers to validate mount requests.

**Inputs:**
- `path: &file::Path`: The directory path requested by a client.

**Outputs:**
- `Option<&ExportEntryWrapper>`: `Some` if the path is exported, `None` otherwise.

**Steps:**
1. Access the `exports` field.
2. Call the `by_path` method on the `ExportRegistry`, which performs a hash map lookup.

**Complexity:**
- **Time**: O(1) average case.
- **Space**: O(1).

**Determinism:**
- **Deterministic**.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::service::mount::mnt`**:
    - **`Mnt` trait implementation**: The `mnt` module relies on `MountService::export_entry` to validate if a requested path exists in the `ExportRegistry`. It also relies on `MountService::mounts` (via the `RwLock`) to insert new `MountEntry` records upon a successful mount.
- **From `nfs_mamont::service::mount::export`**:
    - **`Export` trait implementation**: The `export` module relies on `MountService::exports` to retrieve the full list of exported directories. It assumes the `ExportRegistry` (or `MountService` via delegation) exposes an `export_list()` method.
- **From `nfs_mamont::service::mount::dump`**:
    - **`Dump` trait implementation**: The `dump` module relies on `MountService::mounts` to read the current state of active mounts. It acquires a read lock on the `RwLock` to iterate over the `by_client` map.
- **From `nfs_mamont::service::mount::umnt`**:
    - **`Umnt` trait implementation**: The `umnt` module relies on `MountService::mounts` to mutate the state. It acquires a write lock to remove specific `MountEntry` items from the `HashSet` associated with a client.
- **From `nfs_mamont::service::mount::umntall`**:
    - **`Umntall` trait implementation**: The `umntall` module relies on `MountService::mounts` to clear all mounts for a client. It acquires a write lock to remove the client's key entirely from the `by_client` map.
- **From `nfs_mamont::vfs::file`**:
    - **`Handle` and `Path`**: These types are fundamental to the data model. `ExportEntryWrapper` stores a `file::Handle` (the root file handle), and `ExportRegistry` uses `file::Path` as the key. The validation logic for these types (e.g., max length) is encapsulated within the `vfs::file` module, ensuring that `MountService` only deals with valid paths and handles.

---

## 4. Data Model

Entities:
- **`ExportEntryWrapper`**: A public struct combining a user-defined export configuration with a VFS file handle.
    - `export: ExportEntry`: The directory path and allowed hostnames.
    - `root_handle: file::Handle`: The opaque file handle representing the root of the export.
- **`ExportRegistry`**: A private struct acting as a read-only map of available exports.
    - `by_directory: HashMap<file::Path, ExportEntryWrapper>`: Maps directory paths to their wrappers.
- **`MountRegistry`**: A private struct tracking active client sessions.
    - `by_client: HashMap<SocketAddr, HashSet<MountEntry>>`: Maps client addresses to the set of directories they have mounted.
- **`MountService`**: The public service struct holding the state.
    - `exports: Arc<ExportRegistry>`: Shared, static configuration.
    - `mounts: RwLock<MountRegistry>`: Shared, dynamic state.

Relations:
- **`MountService` owns `ExportRegistry` and `MountRegistry`**: Composition.
- **`ExportRegistry` maps `file::Path` to `ExportEntryWrapper`**: 1:1 association.
- **`MountRegistry` maps `SocketAddr` to `MountEntry` set**: 1:N association (one client, multiple mounts).

Global Invariants:
- **Export Immutability**: Once `MountService` is created, the `exports` field is never modified. The `Arc` ensures that multiple readers can access it simultaneously without synchronization overhead beyond the atomic reference count.
- **Mount Consistency**: The `mounts` field is protected by a `RwLock`. Any modification to the `by_client` map must occur while holding the write lock, ensuring that concurrent reads see a consistent state.

## 5. Error Model

Error Types:
- This module does not define a specific error enum for its own public methods. The constructor `with_exports` is infallible, and `export_entry` returns `Option` to handle missing keys.

Error Propagation Strategy:
- **Option Type**: Used in `export_entry` to gracefully handle lookup failures (missing exports) without panicking or returning explicit errors. The caller (e.g., the `mnt` module) maps this `None` to a protocol-specific error (e.g., `Fail::Access`).

Recoverability:
- **N/A**: The module itself performs minimal validation (mostly just lookups). Recovery from invalid states is handled by the protocol implementations in the sub-modules.

Panics:
- **Allowed**: No explicit panics in this module.
- **Conditions**: A panic could theoretically occur if the `RwLock` in `MountService` becomes poisoned, but this is a runtime failure of the lock primitive, not explicit logic in this module.

---

## 6. Traits

List which external traits this module implements:
- This module does not directly implement any external traits on the types defined within it. However, it organizes the implementations of `crate::mount::mnt::Mnt`, `crate::mount::export::Export`, `crate::mount::dump::Dump`, `crate::mount::umnt::Umnt`, and `crate::mount::umntall::Umntall` for the `MountService` struct via its sub-modules.

---

## 7. Overview

This module is used in order to **centralize the state management for the MOUNT v3 protocol**, separating the static definition of exported filesystems from the dynamic tracking of client connections. The system contains a complex NFS server implementation where the MOUNT protocol serves as the entry point for clients to obtain file handles. This module is critical because it acts as the authoritative source of truth for "what is available" (exports) and "who is connected" (mounts).

A typical usage scenario of the system involves the server starting up with a configuration file listing exported directories (e.g., `/export/data`, `/home`). The main initialization logic converts this configuration into a list of `ExportEntryWrapper` objects (which includes looking up the VFS file handles for these paths) and passes it to `MountService::with_exports`. The resulting `MountService` instance is then shared with the RPC task handlers. When a client sends a `MNT` request for `/export/data`, the handler (in the `mnt` sub-module) queries `MountService` to verify the path exists and to retrieve the pre-calculated file handle. If successful, the handler updates the `MountService`'s internal registry to record that the client has mounted this path.

Inside the system, the following things happen and they use this module:
1. **Access Control Enforcement**: The `ExportRegistry` ensures that clients can only mount directories explicitly defined by the administrator. By storing the `root_handle` alongside the export, the system ensures that the client receives a valid, stable file handle that corresponds to the VFS layer's view of that directory.
2. **Session Tracking**: The `MountRegistry` allows the server to maintain a list of active mounts. This is essential for administrative tools (like `showmount`) and for handling client disconnections gracefully (e.g., via `UMNTALL`).
3. **Concurrency Management**: By using `Arc` for exports and `RwLock` for mounts, the module enables high-performance concurrent access. Multiple clients can query the export list simultaneously without blocking, while mount/unmount operations are serialized to prevent race conditions.

Without this module, the MOUNT protocol handlers would lack a shared context. They would have no way to know which directories are valid exports or which clients are currently mounted, leading to a broken or insecure implementation. This module provides the necessary data structures and lifecycle management to support the full MOUNT v3 protocol specification.