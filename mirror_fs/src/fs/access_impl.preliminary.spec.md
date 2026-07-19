<!-- SPEC_HASH: 09f6b223ba43f9c68dda6cca5a994b39259d9fb02fed64ce671b3d0581799edb -->
# Module Specification

Module: mirrorfs::fs::access_impl
Rust File: mirror_fs/src/fs/access_impl.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`nfs_mamont::vfs::access`**: Used to import the `Access` trait, which this module implements for the `MirrorFS` struct. It also provides the `Args`, `Success`, `Fail`, and `Mask` types that define the input and output contract for the access check operation.
- **`nfs_mamont::vfs::file`**: Used to import the `file::Attr` and `file::Type` types. These are necessary to inspect the file's metadata (specifically mode bits and file type) to determine if requested permissions should be granted.
- **`super::MirrorFS`**: The parent struct for which the `Access` trait is being implemented. The implementation relies on methods defined on `MirrorFS` (assumed to exist, such as `path_for_handle`, `metadata`, and `attr_from_metadata`) to bridge the gap between abstract file handles and concrete file system metadata.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

### Mechanism 1: Implementation of `Access` trait for `MirrorFS`

**Intent:**
To provide the concrete logic for the NFSv3 `ACCESS` procedure within the `MirrorFS` backend. This involves resolving an opaque file handle to a concrete path, retrieving the file's metadata, and calculating the effective access rights based on that metadata.

**Inputs:**
- `args: access::Args`: Contains the `file::Handle` of the object to check and the `access::Mask` representing the permissions requested by the client.

**Outputs:**
- `Result<access::Success, access::Fail>`: 
 - `Success`: Contains the `access::Mask` of permissions granted and the `file::Attr` of the object.
 - `Fail`: Contains a `vfs::Error` if the handle cannot be resolved or metadata cannot be read.

**Steps:**
1. **Handle Resolution**: Calls `self.path_for_handle(&args.file).await`. This is an assumed method on `MirrorFS` that translates a file handle into a filesystem path. If this fails, the error is returned immediately in `access::Fail`.
2. **Metadata Retrieval**: Calls `Self::metadata(&path)`. This is an assumed associated method on `MirrorFS` that performs a stat operation on the path. If this fails, the error is returned immediately in `access::Fail`.
3. **Attribute Conversion**: Calls `Self::attr_from_metadata(&meta)`. This is an assumed method that converts the raw OS metadata into the standardized `file::Attr` structure.
4. **Permission Calculation**: Calls `Self::compute_access_mask(&attr, args.mask)` to determine which bits in the requested mask are granted based on the file's mode.
5. **Response Construction**: Returns `Ok(access::Success { object_attr: Some(attr), access: granted })`.

**Edge Cases:**
- **Handle Resolution Failure**: If `path_for_handle` returns an error (e.g., stale handle), `object_attr` in the returned `Fail` struct is set to `None`.
- **Metadata Read Failure**: If `metadata` fails (e.g., I/O error), `object_attr` is also `None`.

**Complexity:**
- **Time**: Dependent on the underlying `path_for_handle` and `metadata` operations (typically O(1) or O(depth) for path resolution, O(1) for stat).
- **Space**: O(1) additional space beyond the structures returned.

**Determinism:**
- **Deterministic**: Given the same file system state and inputs, the output is deterministic.

### Mechanism 2: Access Mask Computation (`compute_access_mask`)

**Intent:**
To determine which specific access permissions are granted based on the file's Unix mode bits. This function implements a simplified permission check that considers only the **owner** class of permissions.

**Inputs:**
- `attr: &file::Attr`: The attributes of the file, containing the `mode` bits and `file_type`.
- `requested: access::Mask`: The bitmask of permissions requested by the client.

**Outputs:**
- `access::Mask`: A bitmask containing only the permissions that are granted.

**Steps:**
1. Extracts the Unix mode bits from `attr.mode`.
2. Determines if the file is a directory by checking `attr.file_type`.
3. Checks the owner permission bits:
 - `owner_r`: Read permission (bit 0o400).
 - `owner_w`: Write permission (bit 0o200).
 - `owner_x`: Execute/search permission (bit 0o100).
4. Iterates through the requested permissions and sets bits in the result `u32` if:
 - `READ` is requested AND `owner_r` is true.
 - `LOOKUP` is requested AND the file is a directory AND `owner_x` is true.
 - `MODIFY`, `EXTEND`, or `DELETE` are requested AND `owner_w` is true.
 - `EXECUTE` is requested AND `owner_x` is true.
5. Constructs a new `access::Mask` from the resulting `u32` using `from_wire`.

**Edge Cases:**
- **Directory Lookup**: The `LOOKUP` permission is explicitly tied to the execute bit on the mode and requires the file type to be `Directory`. For non-directories, `LOOKUP` is not granted even if execute is set.
- **Write Operations**: `MODIFY`, `EXTEND`, and `DELETE` are all mapped to the single write permission bit (`owner_w`).

**Complexity:**
- **Time**: O(1).
- **Space**: O(1).

**Determinism:**
- **Deterministic**.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::access`**:
 - **`Access` Trait**: The module implements this trait, specifically the `access` async method, fulfilling the contract required by the VFS layer.
 - **`Mask`**: The module uses `Mask` to interpret the requested permissions and to construct the granted permissions mask. It relies on `Mask::from_wire` to ensure the returned mask is valid.
 - **`Success` and `Fail`**: The module constructs these structs to return the result of the access check, populating the `object_attr` field with the retrieved metadata.

- **From `nfs_mamont::vfs::file`**:
 - **`Attr`**: The module uses the `mode` and `file_type` fields of `Attr` to perform the permission logic. It assumes `Attr` is populated by the `attr_from_metadata` helper.
 - **`Type::Directory`**: The module matches against this variant to determine if `LOOKUP` permissions should be evaluated.

---

## 4. Data Model

Entities:
- This module defines no new public entities. It operates on `access::Args`, `access::Success`, `access::Fail`, `access::Mask`, and `file::Attr` defined in dependencies.

Relations:
- **Implementation**: `MirrorFS` implements `access::Access`.

Global Invariants:
- **Owner-Only Permissions**: The `compute_access_mask` function enforces an invariant that access rights are derived strictly from the owner bits of the file mode (bits 0o700). Group and world bits are ignored.
- **Attribute Availability**: In the `Ok` branch of `access`, the `object_attr` field is always `Some`. In the `Err` branch, it is always `None`.

## 5. Error Model

Error Types:
- **`vfs::Error`**: The error type returned within `access::Fail`. This type is assumed to be returned by the helper methods `path_for_handle` and `metadata` on `MirrorFS`.

Error Propagation Strategy:
- **Early Return**: If `path_for_handle` or `metadata` return an error, the `access` function immediately returns `Err(access::Fail { error, object_attr: None })`. It does not attempt to recover or provide partial attributes on failure.

Recoverability:
- **Dependent on Error**: Recoverability depends on the specific `vfs::Error` variant returned by the underlying `MirrorFS` methods (e.g., `StaleFile` vs `IO`).

Panics:
- **Allowed**: No.
- **Conditions**: The code performs no explicit unwrapping or operations that should panic. It relies on `?` for error propagation.

---

## 6. Traits

List which external traits this module implements:
- **`nfs_mamont::vfs::access::Access`**: Implemented for `MirrorFS`. This provides the asynchronous `access` method used by the NFS server to check file permissions.

List which traits this module defines:
- None.

---

## 7. Overview

This module is used in order to **implement the permission checking logic for the MirrorFS backend**, bridging the generic NFSv3 `ACCESS` interface with the specific behavior of the underlying file system. The system contains a complex architecture where the VFS layer defines abstract operations (like checking access via a handle), but the storage backend (MirrorFS) must translate these handles into concrete OS-level checks. This module is necessary because it defines *how* MirrorFS interprets file permissions to satisfy the NFS protocol requirements.

A typical usage scenario of the system involves an NFS client requesting to verify if it can write to a file before attempting a write operation. The RPC layer receives the request and calls the `access` method on the `MirrorFS` instance. This module's implementation takes over: it resolves the opaque file handle to a local path (using assumed `MirrorFS` internals), reads the file's metadata from the disk, and applies the logic in `compute_access_mask`. This logic checks the Unix mode bits of the file to see if the owner has write permission. If yes, it returns a success with the `MODIFY` bit set; otherwise, it returns a success with that bit unset.

Inside the system, the following things happen and they use this module:
1. **Permission Mapping**: The module maps high-level NFS access rights (READ, LOOKUP, MODIFY, etc.) to low-level Unix file mode bits (r, w, x). This mapping is crucial because NFS concepts like "EXTEND" (appending to a file) do not have direct Unix equivalents; this module maps them to the generic "write" permission.
2. **Handle Translation**: The module initiates the translation of an NFS file handle into a usable filesystem path via `path_for_handle`. This is the critical step that connects the stateless NFS protocol to the stateful local filesystem.
3. **Attribute Caching**: By returning the file's attributes in the `Success` object, this module supports the NFS recommendation to return attributes along with access results, allowing the client to update its cache without a separate `GETATTR` call.

**Assumptions**:
- The `MirrorFS` struct (defined in `super`) provides the following methods, which are not visible in the provided snippet but are required for this code to compile:
 - `async fn path_for_handle(&self, handle: &file::Handle) -> Result<std::path::PathBuf, vfs::Error>`
 - `fn metadata(path: &std::path::Path) -> Result<std::fs::Metadata, vfs::Error>` (or similar return type compatible with `attr_from_metadata`)
 - `fn attr_from_metadata(meta: &std::fs::Metadata) -> file::Attr`
- The error type returned by `path_for_handle` and `metadata` is compatible with `vfs::Error`.