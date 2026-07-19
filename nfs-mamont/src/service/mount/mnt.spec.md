<!-- SPEC_HASH: 68e706d1384f12011c039845c7305ed2c2d3c54e413c3cd7ff45b3c0d799e457 -->
# Module Specification

Module: nfs_mamont::service::mount::mnt
Rust File: src/service/mount/mnt.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`std::net::SocketAddr`**: Used to identify the client's network address. This serves as the key for tracking active mounts in the registry and is converted to a string for hostname validation.
- **`tracing::warn`**: Used to emit structured log events when a mount request is denied. The log includes the requested path, client address, and the list of configured exports to aid debugging.
- **`crate::mount::mnt::{Args, Fail, Mnt, Success}`**: Used to implement the `Mnt` trait for `MountService`. `Args` provides the requested directory, `Success` and `Fail` define the return types, and `Mnt` is the trait being implemented.
- **`crate::mount::{HostName, MountEntry}`**: Used to construct the record of the active mount. `HostName` wraps the validated client IP string, and `MountEntry` links this hostname to the mounted directory.
- **`crate::rpc::OpaqueAuth`**: Used as a parameter in the `mnt` function signature to satisfy the `Mnt` trait definition, although the credentials are not explicitly validated in this specific implementation logic.
- **`super::MountService`**: The struct for which the `Mnt` trait is being implemented. It provides access to the export configuration and the mount registry.
- **`super::AUTH`**: *Assumption*: A constant defined in the parent module (`service/mount/mod.rs`) representing the list of supported authentication flavors. It is converted to a `Vec` to be included in the success response.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To implement the `Mnt` procedure of the MOUNT v3 protocol for the `MountService`.
- To validate client requests against the server's export configuration.
- To record successful mount attempts in the server's runtime state (mount registry) to track which clients have mounted which directories.

Inputs:
- `args: Args`: Contains the `dirpath` (`file::Path`) requested by the client.
- `client_addr: SocketAddr`: The network address of the client making the request.
- `_cred: OpaqueAuth`: Authentication credentials (ignored in this implementation).

Outputs:
- `Result<Success, Fail>`:
 - `Ok(Success)`: Contains the file handle for the directory and the list of supported authentication flavors.
 - `Err(Fail::Access)`: Returned if the requested directory is not found in the export list.
 - `Err(Fail::Inval)`: Returned if the client's IP address cannot be converted to a valid `HostName`.

Steps:
1. **Export Lookup**: The method calls `self.export_entry(&args.dirpath).await` to check if the requested path exists in the server's export configuration.
2. **Access Denial**: If the lookup returns `None`:
 - The method retrieves the list of all configured exports.
 - It logs a warning message containing the requested path, client address, and the list of valid exports.
 - It returns `Err(Fail::Access)`.
3. **Handle Extraction**: If the export is found (`Some(export)`), the method clones the `root_handle` from the export entry.
4. **Hostname Resolution**: The method attempts to create a `HostName` from the client's IP address (`client_addr.ip().to_string()`). If this fails (e.g., the string representation is invalid), it returns `Err(Fail::Inval)`.
5. **Mount Entry Creation**: A `MountEntry` is created containing the resolved `hostname` and the requested `directory` (cloned from `args.dirpath`).
6. **State Update**: The method acquires a write lock on `self.mounts`. It then accesses the `by_client` map, retrieves or creates the `HashSet` for the `client_addr`, and inserts the new `mount_entry`.
7. **Success Response**: The method returns `Ok(Success { file_handle, auth_flavors: AUTH.to_vec() })`.

Edge Cases:
- **Missing Export**: The implementation treats a missing export as an access error (`Fail::Access`) rather than "No such file or directory" (`Fail::NoEnt`), effectively hiding the existence of non-exported paths.
- **Hostname Validation**: If the string representation of the client IP exceeds the maximum length defined for `HostName`, the mount fails with `Fail::Inval`.

Complexity:
- **Time**:
 - `export_entry`: O(1) average case (hash map lookup).
 - `mounts.write()`: O(1) average case for map access and set insertion.
 - Overall: O(1) amortized.
- **Space**: O(1) for the operation itself, though it adds one entry to the in-memory `MountRegistry`.

Determinism:
- **Deterministic**: The logic follows a strict sequence of checks and state mutations. The only non-deterministic aspect is the scheduling of the asynchronous lock acquisition (`write().await`).

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::service::mount::mod.rs`**:
 - **`MountService::export_entry`**: This module relies on this method to perform the lookup of the requested directory path against the static export configuration.
 - **`MountService::mounts`**: This module relies on the `RwLock`-protected `mounts` field to safely update the list of active mounts. It specifically uses the `by_client` structure within the registry.
 - **`ExportEntryWrapper`**: The module assumes the returned export entry contains a `root_handle` field which is cloned to form the response.

- **From `nfs_mamont::mount::mnt`**:
 - **`Mnt` Trait**: The module implements this trait, adhering to its signature and error contract (`Result<Success, Fail>`).

- **From `nfs_mamont::mount`**:
 - **`HostName::new`**: The module uses this constructor to validate the client's IP string. It maps the potential `io::Error` to the MOUNT protocol error `Fail::Inval`.

---

## 4. Data Model

Entities:
- This module defines no new public entities. It operates on instances defined in its dependencies (`MountService`, `Args`, `Success`, `Fail`, `HostName`, `MountEntry`).

Relations:
- **`MountService` implements `Mnt`**: This module provides the implementation logic for the `mnt` method defined in the `mnt` module.
- **`MountService` owns `MountRegistry`**: The implementation mutates the `mounts` field of `MountService` by adding `MountEntry` records.

Global Invariants:
- **Mount Registry Consistency**: After a successful `mnt` call, the `MountRegistry` must contain a `MountEntry` linking the `client_addr` to the requested `directory`.
- **Export Immutability**: The implementation reads from the export configuration but does not modify it.

## 5. Error Model

Error Types:
- **`Fail::Access`**: Returned when the requested directory is not present in the `exports` list of the `MountService`.
- **`Fail::Inval`**: Returned when the client's IP address cannot be converted into a valid `HostName` (e.g., if the string is too long).

Error Propagation Strategy:
- **Manual Mapping**: The module manually maps internal conditions to the `Fail` enum variants defined in the `mnt` module. It does not use `?` for propagation but returns `Err(...)` explicitly.

Recoverability:
- **Recoverable**: The client receives a specific error code indicating why the mount failed. The server state remains unchanged (no entry is added to the mount registry) if an error is returned.

Panics:
- **Allowed**: No explicit panics.
- **Conditions**: A panic could theoretically occur if the `RwLock` on `self.mounts` is poisoned, but this is a runtime failure of the lock, not explicit code in this module.

---

## 6. Traits

List which external traits this module implements:
- **`crate::mount::mnt::Mnt`**: Implemented for `super::MountService`.

---

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to **enforce access control and maintain the session state for the MOUNT protocol**. It acts as the gatekeeper for the NFS server, determining whether a client is permitted to access a specific directory and, if so, initializing the session record.

The system contains a complex implementation of an NFS server where the MOUNT protocol is the mechanism by which clients translate a human-readable path (e.g., "/export/data") into an opaque file handle used by the NFS protocol. This module is critical because it bridges the gap between the static configuration (what directories are exported) and the dynamic runtime state (who is currently mounted). Without this module, the server would have no way to verify if a path is valid for a specific client or to track active connections for administrative purposes (like the `DUMP` procedure).

A typical usage scenario of the system involves a client sending a `MNT` request for "/home/user". The RPC dispatcher routes this to the `mnt` method implemented in this module. The module checks the `MountService`'s export list. If "/home/user" is exported, the module retrieves the pre-calculated file handle, creates a record linking the client's IP to "/home/user", and stores it in the `MountService`'s mount registry. The client receives the file handle and proceeds to use the NFS protocol. If "/home/user" is not exported, the module logs the attempt and returns an access error, preventing the client from proceeding.

Inside the system, the following things happen and they use this module:
1. **Authorization**: The `export_entry` lookup serves as the authorization check. The system relies on this module to ensure that only paths explicitly defined in the `ExportRegistry` are accessible.
2. **Session Tracking**: By inserting into `self.mounts`, this module populates the data source for the `DUMP` procedure (implemented in a sibling module), allowing administrators to see which clients are mounted.
3. **Error Feedback**: The logging mechanism here provides the primary visibility into failed mount attempts, which is essential for security auditing and troubleshooting client configuration issues.