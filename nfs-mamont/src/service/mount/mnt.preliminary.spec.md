<!-- SPEC_HASH: 68e706d1384f12011c039845c7305ed2c2d3c54e413c3cd7ff45b3c0d799e457 -->
# Module Specification

Module: nfs_mamont::service::mount::mnt
Rust File: src/service/mount/mnt.rs

---

## 1. Dependencies

From the code analysis, the following external crates and standard library modules are used:

- **`std::net::SocketAddr`**: Used to identify the client's network address. This is essential for logging security events (denied mounts) and for recording the mount entry in the server's state table.
- **`tracing`**: Used for the `warn!` macro to log details when a mount request is denied. This aids in debugging and security auditing by recording the requested path, client IP, and available exports.
- **`crate::mount::mnt`**: Used to import the `Mnt` trait, `Args`, `Success`, and `Fail` types. This module implements the `Mnt` trait, effectively providing the business logic for the MOUNT protocol procedure defined in the dependency.
- **`crate::mount`**: Used to import `HostName` and `MountEntry`. These types are used to construct the record of the mount operation that is stored in the server's state.
- **`crate::rpc`**: Used to import `OpaqueAuth`. While the credential argument is accepted by the interface, it is currently unused in the implementation logic (`_cred`).
- **`super::MountService`**: The parent module struct for which this implementation is written. It is assumed to hold the server state, specifically the export configuration and the active mount table.
- **`super::AUTH`**: A constant defined in the parent module, assumed to be a slice of `AuthFlavor` representing the authentication methods supported by the server.

---

## 2. Mechanics

This module implements the server-side logic for the MOUNT v3 `MNT` procedure. It acts as the bridge between the abstract RPC interface and the concrete state management of the NFS server.

Intent:
- To validate incoming mount requests against the server's export configuration.
- To generate and return the appropriate file handle for a valid export.
- To maintain a registry of which clients have mounted which directories.
- To provide logging for security-relevant events (access denials).

Inputs:
- `args: Args`: Contains the directory path (`dirpath`) the client wishes to mount.
- `client_addr: SocketAddr`: The network address of the client making the request.
- `_cred: OpaqueAuth`: Authentication credentials (currently ignored in this implementation).

Outputs:
- `Result<Success, Fail>`: On success, contains the file handle and supported auth flavors. On failure, contains a MOUNT protocol error code.

Steps:
1. **Export Lookup**: The method calls `self.export_entry(&args.dirpath).await` to check if the requested path corresponds to a configured export.
2. **Access Control**: If the export lookup returns `None`:
   - The method retrieves the list of all configured exports via `self.exports.export_list()`.
   - It logs a warning message containing the requested path, client IP, and the list of valid exports.
   - It returns `Err(Fail::Access)`.
3. **Handle Extraction**: If the export exists, the method clones the `root_handle` from the export entry.
4. **Client Identification**: The method attempts to create a `HostName` from the client's IP address string. If this conversion fails, it returns `Err(Fail::Inval)`.
5. **State Update**: The method constructs a `MountEntry` containing the client's hostname and the requested directory. It then acquires a write lock on `self.mounts` and inserts this entry into the set associated with the `client_addr`.
6. **Success Response**: The method returns `Ok(Success { file_handle, auth_flavors: AUTH.to_vec() })`, providing the client with the handle and the server's supported authentication flavors.

Edge Cases:
- **Invalid Export Path**: The requested path is not in the export list. Handled by returning `Fail::Access`.
- **Invalid IP Address**: The client IP cannot be converted to a valid hostname string. Handled by returning `Fail::Inval`.

Complexity:
- Time: Depends on the complexity of `export_entry` lookup (assumed O(1) or O(log N)) and the locking mechanism for `mounts`.
- Space: O(1) per request for the immediate operation, plus the space required to store the new `MountEntry` in the global state.

Determinism:
- Deterministic. Given the same internal state (exports, mounts) and inputs, the output and state transitions are identical.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::mount::mnt`**:
  - **`Mnt` Trait**: Defines the asynchronous `mnt` function signature that this module implements. It enforces the contract of taking `Args`, `SocketAddr`, and `OpaqueAuth` and returning `Result<Success, Fail>`.
  - **`Fail` Enum**: Provides the specific error variants used for signaling failure conditions to the client, specifically `Fail::Access` (permission denied) and `Fail::Inval` (invalid argument, used here for hostname parsing failure).
  - **`Success` Struct**: Defines the structure of the successful response, requiring a `file_handle` and a list of `auth_flavors`.
- **From `nfs_mamont::mount`**:
  - **`HostName`**: A wrapper type used to represent the client's identity. The module uses `HostName::new` to validate and store the client's IP string.
  - **`MountEntry`**: A data structure used to record the mount event, linking a `HostName` to a directory `Path`.
- **From `nfs_mamont::rpc`**:
  - **`OpaqueAuth`**: The type used to pass authentication data. Although part of the signature, the implementation ignores it (`_cred`), relying solely on IP-based logic in this specific procedure.

---

## 4. Data Model

Entities:
- **`MountService` (Inferred)**: The struct implementing the logic. It is inferred to contain:
  - `exports`: A configuration source providing `export_entry` (lookup by path) and `export_list` (list of all paths).
  - `mounts`: A concurrent map (likely `RwLock<HashMap<SocketAddr, HashSet<MountEntry>>>`) tracking active mounts per client.
- **`ExportEntry` (Inferred)**: Represents a configured export. It is inferred to contain a `root_handle` (the file handle for the root of the export).
- **`MountEntry`**: Represents an active mount session, linking a `HostName` to a directory `Path`.
- **`HostName`**: A validated string wrapper representing a host identifier.

Relations:
- `MountService` manages `ExportEntry` configurations.
- `MountService` manages a collection of `MountEntry` records, keyed by `SocketAddr`.
- `MountEntry` references `HostName` and `Path`.

Global Invariants:
- The `self.mounts` table must accurately reflect all currently active mounts.
- Access to `self.mounts` for modification must be exclusive (enforced by `write().await`).

---

## 5. Error Model

Error Types:
- **`Fail::Access`**: Returned when the requested directory is not found in the server's export list. This indicates the client is not permitted to mount this path.
- **`Fail::Inval`**: Returned when the client's IP address cannot be converted into a valid `HostName`.

Error Propagation Strategy:
- Custom enum (`Fail` from `nfs_mamont::mount::mnt`). Errors are returned directly to the RPC layer to be transmitted back to the client.

Recoverability:
- **`Fail::Access`**: The client may recover by requesting a different path that is actually exported.
- **`Fail::Inval`**: This indicates a server-side configuration or parsing issue with the client's IP string representation. It is likely not recoverable by the client changing the request path alone.

Panics:
- Allowed: No explicit panics in the code. However, standard Rust panics may occur if the internal locks (`self.mounts`) are poisoned or if memory allocation fails.

---

## 6. Traits

The module implements the following traits:

- **`mnt::Mnt` for `MountService`**: Implements the asynchronous `mnt` procedure defined in the `nfs_mamont::mount::mnt` module.

---

## 7. Overview

This module is used in order to provide the concrete server-side logic for the MOUNT protocol's `MNT` procedure, bridging the abstract protocol definition with the server's runtime state. It is responsible for enforcing export policies (access control) and maintaining the server's registry of active mounts.

This system contains the implementation of the `Mnt` trait for `MountService`, which validates client requests against configured exports, generates the necessary file handles, and updates the shared mount table. It ensures that only valid, exported directories are accessible to clients and that every successful mount is recorded for potential administrative use (e.g., unmount notifications).

A typical usage scenario of the system involves a client sending a MOUNT request for a directory (e.g., "/home/user"). The RPC layer dispatches this to the `mnt` function implemented here. The function checks if "/home/user" is defined in `self.exports`. If it is, the function retrieves the pre-calculated file handle for that directory, creates a `MountEntry` associating the client's IP with the path, and stores it in `self.mounts`. Finally, it returns the file handle to the client. If the path is not exported, the function logs the event and returns an `Access` error.

Inside the system, the following things happen and they use:
- **Access Control**: Uses `self.export_entry` to verify if the requested path is available for mounting.
- **State Management**: Uses `self.mounts.write().await` to safely update the server's view of active mounts across concurrent requests.
- **Observability**: Uses `tracing::warn` to log denied access attempts, providing visibility into potential security probing or misconfigurations.