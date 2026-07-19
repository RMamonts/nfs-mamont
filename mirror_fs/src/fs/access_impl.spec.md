<!-- SPEC_HASH: 09f6b223ba43f9c68dda6cca5a994b39259d9fb02fed64ce671b3d0581799edb -->
# Module Specification

Module: mirrorfs::fs::access_impl
Rust File: mirror_fs/src/fs/access_impl.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`nfs_mamont::vfs::access`**: Used to import the `Access` trait, which this module implements for the `MirrorFS` struct. It also provides the `Args`, `Success`, `Fail`, and `Mask` types that define the input and output contract for the access check operation.
- **`nfs_mamont::vfs::file`**: Used to import the `file::Attr` type, which contains the file metadata (specifically `mode` and `file_type`) required to compute access permissions.
- **`super::MirrorFS`**: The struct for which the `Access` trait is being implemented. The implementation relies on methods `path_for_handle`, `metadata`, and `attr_from_metadata` which are assumed to exist on `MirrorFS` (though they were not listed in the provided partial specification for `mirrorfs::fs`).

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

### Mechanism 1: Access Check Implementation (`Access::access`)

**Intent:**
To fulfill the NFSv3 `ACCESS` procedure contract for the `MirrorFS` backend. This involves translating an opaque file handle into a concrete filesystem path, retrieving its metadata, and determining which of the requested permissions are granted based on the file's Unix mode bits.

**Inputs:**
- `&self`: Reference to the `MirrorFS` instance.
- `args: access::Args`: Contains the `file::Handle` to check and the `access::Mask` of requested permissions.

**Outputs:**
- `Result<access::Success, access::Fail>`: Returns a success struct containing the granted access mask and file attributes, or a failure struct containing an error code.

**Steps:**
1. **Handle Resolution**: Calls `self.path_for_handle(&args.file).await` to convert the NFS file handle into a local filesystem path. If this fails, the error is returned immediately in `access::Fail` with `object_attr` set to `None`.
2. **Metadata Retrieval**: Calls `Self::metadata(&path)` to obtain the filesystem metadata (stat) for the resolved path. If this fails, the error is returned immediately in `access::Fail` with `object_attr` set to `None`.
3. **Attribute Conversion**: Calls `Self::attr_from_metadata(&meta)` to convert the raw OS metadata into the standardized `file::Attr` structure used by the VFS layer.
4. **Permission Calculation**: Calls `Self::compute_access_mask(&attr, args.mask)` to determine which permissions are granted based on the file's attributes.
5. **Response Construction**: Returns `Ok(access::Success)` containing the computed mask and the file attributes.

**Edge Cases:**
- **Early Failure**: If either handle resolution or metadata retrieval fails, the operation aborts, and no attributes are returned to the client (`object_attr: None`).

**Complexity:**
- **Time**: Dependent on the underlying filesystem operations (path lookup and `stat`). O(1) relative to the logic within this module, but O(N) relative to path depth in the OS.
- **Space**: O(1) additional space (stack allocation for attributes).

**Determinism:**
- **Deterministic**: Given the same filesystem state and inputs, the output is consistent.

### Mechanism 2: Unix Mode to NFS Mask Mapping (`compute_access_mask`)

**Intent:**
To translate standard Unix permission bits (specifically the owner class bits) into the NFSv3 access mask format. This function acts as the policy engine determining if specific rights (READ, WRITE, EXECUTE, etc.) are available.

**Inputs:**
- `attr: &file::Attr`: The file attributes containing the `mode` (u32) and `file_type`.
- `requested: access::Mask`: The bitmask of permissions the client is asking about.

**Outputs:**
- `access::Mask`: A bitmask containing only the permissions that are granted.

**Steps:**
1. **Extract Mode**: Retrieves the `mode` field from `attr`.
2. **Check Type**: Determines if the file is a directory using `matches!(attr.file_type, file::Type::Directory)`.
3. **Parse Owner Bits**: Checks the Unix owner permission bits:
   - `owner_r`: `mode & 0o400 != 0`
   - `owner_w`: `mode & 0o200 != 0`
   - `owner_x`: `mode & 0o100 != 0`
4. **Evaluate Requests**:
   - **READ**: Granted if requested and `owner_r` is true.
   - **LOOKUP**: Granted if requested, the file is a directory, and `owner_x` is true.
   - **MODIFY, EXTEND, DELETE**: Granted if requested and `owner_w` is true.
   - **EXECUTE**: Granted if requested and `owner_x` is true.
5. **Construct Result**: Aggregates the granted bits into a `u32` and wraps it in `access::Mask::from_wire`.

**Edge Cases:**
- **Simplified Permissions**: The implementation only checks the **owner** bits (octal 0700). Group and world permissions are ignored.
- **Directory Lookup**: The `LOOKUP` permission is explicitly tied to the execute bit on the owner, but only for directories.

**Complexity:**
- **Time**: O(1).
- **Space**: O(1).

**Determinism:**
- **Deterministic**.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::access`**:
 - **`Access` Trait**: Defines the `access` method signature that this module implements. It enforces the structure of arguments (`Args`) and return values (`Result<Success, Fail>`).
 - **`Mask`**: Provides the `from_wire` constructor used to create the resulting access mask and the constants (e.g., `Mask::READ`) used to check specific permissions.

- **From `nfs_mamont::vfs::file`**:
 - **`Attr`**: Provides the `mode` field (Unix permission bits) and `file_type` field which are the sole inputs for the permission calculation logic in `compute_access_mask`.

- **From `super::MirrorFS` (Assumed)**:
 - **`path_for_handle`**: Used to resolve the abstract `file::Handle` to a concrete path. This is critical for locating the file on the local disk.
 - **`metadata`**: Used to perform the system call to get file attributes (e.g., `stat`).
 - **`attr_from_metadata`**: Used to normalize the system attributes into the VFS `Attr` structure.

---

## 4. Data Model

Entities:
- No new entities are defined in this module. It utilizes `access::Args`, `access::Success`, `access::Fail`, `access::Mask`, and `file::Attr` from dependencies.

Relations:
- **Implementation**: `MirrorFS` implements `access::Access`.

Global Invariants:
- **Owner-Only Check**: The `compute_access_mask` function enforces an invariant that access rights are derived solely from the owner permission bits of the Unix mode (0o700). Group and other bits are not evaluated.
- **Directory Lookup**: The `LOOKUP` right is only granted if the target is a directory and the owner has execute permissions.

## 5. Error Model

Error Types:
- **`vfs::Error`**: Wrapped inside `access::Fail`. This error originates from the `path_for_handle` or `metadata` calls within `MirrorFS`.

Error Propagation Strategy:
- **Fail-Fast**: The `access` function returns `Err(access::Fail)` immediately upon encountering an error during handle resolution or metadata fetching. The `object_attr` field in the `Fail` struct is explicitly set to `None` in these error cases.

Recoverability:
- **Dependent on Error**: Recoverability depends on the specific `vfs::Error` returned by the underlying `MirrorFS` methods (e.g., `StaleFile` vs `IO`).

Panics:
- **Allowed**: No. The code uses pattern matching and `Result` propagation to avoid panics.

---

## 6. Traits

List which external traits this module implements:
- **`nfs_mamont::vfs::access::Access`**: Implemented for `MirrorFS`. This provides the asynchronous `access` method required by the VFS layer.

---

## 7. Overview

This module is used in order to **implement the access control logic for the `MirrorFS` backend**, translating the abstract NFSv3 access check requirements into concrete checks against the local filesystem's Unix permissions. The system contains a complex architecture where the VFS layer defines generic interfaces (like `Access`) that must be adapted to the specific semantics of the underlying storage. This module is necessary because `MirrorFS` acts as a bridge between the NFS protocol and the local OS filesystem, and it must determine access rights based on the OS's metadata (mode bits) rather than a proprietary ACL system.

A typical usage scenario of the system involves an NFS client requesting to verify if it has write permission on a file before attempting a write operation. The RPC layer receives the request and invokes the `access` method on the `MirrorFS` instance. This module resolves the file handle to a local path, retrieves the file's `stat` information, and checks the Unix mode bits. Specifically, it checks if the owner write bit (0o200) is set. If so, it grants `MODIFY`, `EXTEND`, and `DELETE` permissions in the response. The client then receives this mask and can proceed or abort based on the result.

Inside the system, the following things happen and they use this module:
1.  **Protocol Translation**: The module translates the bitmask-based NFS access request (e.g., `ACCESS_MODIFY | ACCESS_READ`) into a check against standard Unix file permissions (`mode & 0o200`).
2.  **Handle Resolution**: It relies on the `MirrorFS` internal mechanism (`path_for_handle`) to map the opaque NFS file handle to a real file path, which is a prerequisite for any filesystem operation.
3.  **Simplification of Permissions**: The `compute_access_mask` function implements a specific policy where only the file *owner's* permissions are considered. This simplifies the logic but implies that the NFS server behaves as if the requesting user is always the file owner (or that group/other permissions are irrelevant to the server's authorization logic).

**Uncertainty**: The provided specification for `mirrorfs::fs` (mod.rs) does not list the methods `path_for_handle`, `metadata`, or `attr_from_metadata`. However, the code in `access_impl.rs` invokes these methods on `self` (MirrorFS) and `Self`. Therefore, this specification assumes these methods exist and behave as their names suggest (resolving handles, fetching metadata, and converting attributes).