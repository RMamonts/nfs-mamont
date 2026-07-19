<!-- SPEC_HASH: b6efefa10882c2b2bb1546eabf9fcdb901617426e21e8419f9fbda1f28f98d43 -->
# Module Specification

Module: nfs_mamont::service::mount
Rust File: src/service/mount/mod.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`std::collections::{HashMap, HashSet}`**: Used to implement the internal storage mechanisms. `HashMap` provides O(1) lookup for exports by path and mounts by client address. `HashSet` is used to store the list of directories mounted by a specific client, ensuring uniqueness.
- **`std::net::SocketAddr`**: Used as the key in the `MountRegistry` to uniquely identify client connections for tracking active mounts.
- **`std::sync::Arc`**: Used to wrap the `ExportRegistry`. This allows the export configuration (which is read-only after initialization) to be shared efficiently across multiple asynchronous tasks or threads without cloning the underlying data structure.
- **`tokio::sync::RwLock`**: Used to wrap the `MountRegistry`. This provides thread-safe access to the mutable state of active mounts, allowing multiple concurrent readers (e.g., `DUMP` requests) while exclusive access is required for writers (e.g., `MNT`, `UMNT`).
- **`crate::mount::{ExportEntry, MountEntry}`**: Used as the primary data structures for defining server configuration (`ExportEntry`) and recording active client sessions (`MountEntry`).
- **`crate::rpc::AuthFlavor`**: Used to define the authentication flavors supported by the server. The module currently defines a constant `AUTH` using this type.
- **`crate::vfs::file`**: Used for `file::Handle` (the file handle returned to clients) and `file::Path` (the key for looking up exports).
- **Sub-modules (`dump`, `export`, `mnt`, `umnt`, `umntall`)**: These modules contain the implementations of the specific MOUNT v3 RPC procedures. They depend on `MountService` (defined in this module) to access the shared state (`exports` and `mounts`).

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To provide a centralized, thread-safe container for the MOUNT protocol state that separates static configuration (exports) from dynamic runtime state (active mounts).
- To initialize the service with a pre-calculated mapping of directory paths to file handles, abstracting away the VFS lookup logic from the RPC handlers.

Inputs:
- `entries: Vec<ExportEntryWrapper>`: A vector of wrappers containing the export metadata and the corresponding root file handle for each exported directory.

Outputs:
- `Self`: An instance of `MountService` containing the initialized registries.

Steps:
1. **Initialization (`MountService::with_exports`)**:
 - The method accepts a vector of `ExportEntryWrapper`.
 - It creates a `HashMap` (`by_directory`) mapping `file::Path` to `ExportEntryWrapper`.
 - It wraps this map in an `ExportRegistry` and then an `Arc` to create a shared, immutable reference (`exports`).
 - It initializes a default `MountRegistry` (empty map) and wraps it in a `RwLock` (`mounts`).
 - It returns the constructed `MountService`.
2. **Export Lookup (`export_entry`)**:
 - The method provides access to the `exports` registry.
 - It takes a reference to a `file::Path` and returns an `Option<&ExportEntryWrapper>`.
 - This method is asynchronous (`async fn`) to match the interface expected by the sub-module implementations, even though the underlying `HashMap` access is synchronous.

Edge Cases:
- **Duplicate Exports**: If the input `entries` vector contains multiple entries for the same directory path, the `from_entries` function will process them sequentially. The standard `HashMap::insert` behavior will result in the last entry for a given path overwriting previous ones.
- **Concurrent Access**: The `RwLock` on `mounts` ensures that concurrent `MNT` requests are serialized, while `DUMP` requests can proceed in parallel as long as no write operation is in progress.

Complexity:
- **Time**:
 - `with_exports`: O(N), where N is the number of export entries, due to iterating and inserting into the `HashMap`.
 - `export_entry`: O(1) average time complexity for the hash map lookup.
- **Space**:
 - O(N) for storing the export registry.
 - O(M) for storing the mount registry, where M is the number of active mounts.

Determinism:
- Deterministic. The state transitions are fully determined by the sequence of RPC calls received and the initial configuration.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::service::mount::mnt`**:
 - **State Access**: The `mnt` module implementation calls `self.export_entry(&args.dirpath).await` to verify if a path is exported. It relies on the `MountService` defined in this module to provide this method.
 - **State Mutation**: The `mnt` module calls `self.mounts.write().await` to insert new `MountEntry` records. This requires `MountService` to expose a `mounts` field compatible with `tokio::sync::RwLock`.

- **From `nfs_mamont::service::mount::dump`**:
 - **State Reading**: The `dump` module implementation calls `self.mounts.read().await` to retrieve the list of active mounts. It relies on the internal structure of the locked data to have a `by_client` field.

- **From `nfs_mamont::service::mount::umnt` and `umntall`**:
 - **State Cleanup**: These modules rely on `self.mounts.write().await` to remove entries from the `by_client` map.

- **From `nfs_mamont::vfs::file`**:
 - **Path Keying**: The `ExportRegistry` uses `file::Path` as the key. This implies that the `ExportEntryWrapper` must contain a `file::Path` accessible via `entry.export.directory`.

---

## 4. Data Model

Entities:
- **`ExportEntryWrapper`**: A public struct that aggregates an `ExportEntry` (user-visible export data) with a `root_handle` (the VFS file handle for the root of the export).
- **`ExportRegistry`**: A private struct holding a `HashMap<file::Path, ExportEntryWrapper>`. It represents the static configuration of the server.
- **`MountRegistry`**: A private struct holding a `HashMap<SocketAddr, HashSet<MountEntry>>`. It represents the dynamic state of active client mounts.
- **`MountService`**: The public service struct containing `exports: Arc<ExportRegistry>` and `mounts: RwLock<MountRegistry>`.

Relations:
- **`MountService` owns `ExportRegistry`**: Shared via `Arc`.
- **`MountService` owns `MountRegistry`**: Shared via `RwLock`.
- **`ExportRegistry` maps `file::Path` to `ExportEntryWrapper`**: 1-to-1 mapping for exported directories.
- **`MountRegistry` maps `SocketAddr` to `HashSet<MountEntry>`**: 1-to-many mapping (one client can mount multiple directories).

Global Invariants:
- The `exports` field is immutable after the construction of `MountService`.
- Access to `mounts` must be guarded by the `RwLock` to prevent data races in an asynchronous context.

## 5. Error Model

Error Types:
- None defined directly in this module. The constructor `with_exports` is infallible. The lookup method `export_entry` returns `Option`.

Error Propagation Strategy:
- N/A for this module. Errors (like access denied or invalid paths) are handled and returned by the sub-modules (e.g., `mnt::Fail`).

Recoverability:
- N/A.

Panics:
- Allowed: No explicit panics.
- Conditions: Potential panic if the `RwLock` on `mounts` becomes poisoned (e.g., if a panic occurs while holding the lock), though this is a runtime failure mode of the lock itself, not explicit code in this module.

---

## 6. Traits

List which external traits this module implements:
- None. This module defines the struct `MountService` which is used by sub-modules to implement traits like `mnt::Mnt`, `export::Export`, etc.

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here you MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to **provide the central state management and structural backbone for the MOUNT v3 protocol service** within the NFS server. It acts as the container that holds the "truth" about what the server offers (exports) and who is currently connected (mounts).

The system contains a complex implementation of an NFS server where the MOUNT protocol is responsible for the initial handshake between a client and the server. This module is necessary because it decouples the *configuration* of the server (which directories are shared) from the *runtime logic* of the protocol (handling RPCs). By wrapping the export configuration in an `Arc`, the module ensures that the static data can be accessed concurrently by hundreds of clients without synchronization overhead. By wrapping the active mounts in a `RwLock`, it ensures that the dynamic state remains consistent while allowing high throughput for read operations like `DUMP`.

A typical usage scenario of the system involves the server application starting up. The administrator configures a list of directories to export. The application generates file handles for these directories (using the VFS layer) and wraps them in `ExportEntryWrapper` objects. It then instantiates `MountService::with_exports`. From this point on, the RPC dispatcher (likely in `lib.rs`) uses this `MountService` instance to handle incoming MOUNT requests. When a client asks to mount "/export/data", the `mnt` sub-module logic (implemented in a sibling file) runs, querying the `MountService` to see if "/export/data" is in the `exports` map. If it is, the `MountService` provides the pre-calculated file handle, and the `mnt` logic records the client's IP in the `mounts` map.

Inside the system, the following things happen and they use this module:
1. **Service Construction**: The main server loop uses `MountService::with_exports` to create the service object that will be passed to the global MOUNT task.
2. **Protocol Dispatch**: The sub-modules (`mnt`, `umnt`, `export`, etc.) use the `MountService` as the context (`self`) for their trait implementations. They rely on the specific field names (`exports`, `mounts`) and types (`Arc`, `RwLock`) defined here to perform their logic.
3. **Resource Isolation**: The module enforces the architectural decision that file handle resolution for mount roots happens at service initialization time (via `ExportEntryWrapper`), rather than at request time. This simplifies the RPC handlers by removing the need for VFS lookups during the `MNT` procedure.

Without this module, the MOUNT protocol implementation would lack a centralized place to manage its state, leading to potential data races, duplicated configuration logic, and a lack of clear separation between the data model and the RPC procedure handlers.