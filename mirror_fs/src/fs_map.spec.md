<!-- SPEC_HASH: 324dbfe31c5698548cb2da8774b9ecf2b5bf0c1ea7117bca12857a20e1b79cf1 -->
# Module Specification

Module: mirrorfs::fs_map
Rust File: src/fs_map.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`std::collections::{BTreeSet, HashMap}`**: Used to maintain the bidirectional mappings between file system paths, inode-based keys, and integer handles. `BTreeSet` is specifically used for storing multiple paths (hardlinks) associated with a single object key, providing deterministic iteration order.
- **`std::os::unix::fs::MetadataExt`**: Used to retrieve the unique device identifier (`dev`) and inode number (`ino`) from file system metadata. These values form the `ObjectKey`, which serves as the canonical identity for a file or directory, independent of its path.
- **`std::path::{Path, PathBuf}`**: Used for representing and manipulating file system paths. The module distinguishes between absolute paths (inputs/outputs) and relative paths (internal storage relative to the configured root).
- **`nfs_mamont::vfs::file`**: Used for the `file::Handle` type, which acts as the opaque identifier exchanged with the VFS layer. The module encodes internal integer IDs into this type.
- **`nfs_mamont::vfs`**: Used for the `vfs::Error` enum, which provides the error vocabulary (e.g., `StaleFile`, `BadFileHandle`, `NoEntry`) required to signal failures to the upper layers of the NFS stack.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

### Mechanism 1: Handle Registration and Identity Resolution (`ensure_handle_for_path`)

**Intent:**
To translate a concrete file system path into an opaque, stable `file::Handle` suitable for the NFS protocol. This mechanism ensures that multiple paths pointing to the same inode (hardlinks) resolve to the same handle, and that new files receive unique identifiers.

**Inputs:**
- `path: &Path`: The absolute path to the file or directory.

**Outputs:**
- `Result<file::Handle, vfs::Error>`: The opaque handle representing the file, or an error if the path is invalid or inaccessible.

**Steps:**
1. **Normalization**: Strips the configured `root` prefix from the input path to obtain a relative path. If the result is empty, the path is the root itself, and the reserved root handle (ID 1) is returned.
2. **Key Extraction**: Calls `symlink_metadata` on the full path to retrieve the `dev` and `ino` numbers, constructing an `ObjectKey`.
3. **Lookup**: Checks if the `ObjectKey` already exists in `key_to_id`.
4. **Existing Entry**: If the key exists, the new relative path is added to the `key_to_paths` set (to track the hardlink), and the `relative_to_key` map is updated. The existing ID is returned.
5. **New Entry**: If the key is new, a new ID is generated from `next_id` (with wrapping logic to avoid 0 and 1). The new mappings are inserted into `id_to_key`, `key_to_id`, `key_to_paths`, and `relative_to_key`.

**Edge Cases:**
- **Root Path**: Explicitly handled to return ID 1 without accessing the disk.
- **ID Wrapping**: If `next_id` overflows or wraps to 0/1, it is reset to 2 to prevent collisions with reserved IDs.
- **I/O Errors**: Failures to read metadata (e.g., file not found) are mapped to `vfs::Error`.

**Complexity:**
- Time: O(1) average for hash map operations, plus O(K) for inserting into the `BTreeSet` where K is the number of hardlinks.
- Space: O(1) additional space per unique file.

**Determinism:**
- Deterministic, assuming the underlying file system state (metadata) does not change between the start and end of the call.

### Mechanism 2: Handle to Path Resolution (`path_for_handle`)

**Intent:**
To reverse the mapping from an opaque `file::Handle` back to a usable file system path. This mechanism prioritizes "live" paths by checking the disk, which is crucial for handling stale handles or deleted hardlinks gracefully.

**Inputs:**
- `handle: &file::Handle`: The opaque handle to resolve.

**Outputs:**
- `Result<PathBuf, vfs::Error>`: The absolute path to the file, or `StaleFile` if the handle is invalid or no corresponding path exists on disk.

**Steps:**
1. **Decoding**: Decodes the handle bytes into a `u64` ID. Returns `BadFileHandle` if the ID is 0.
2. **Root Check**: If the ID is 1, returns the configured `root` path immediately.
3. **Key Lookup**: Retrieves the `ObjectKey` associated with the ID from `id_to_key`. Returns `StaleFile` if not found.
4. **Path Iteration**: Retrieves the set of relative paths associated with the key from `key_to_paths`.
5. **Validation**: Iterates over the relative paths. For each, it constructs the full path and checks `symlink_metadata`.
6. **Return**: Returns the first full path where the metadata check succeeds.
7. **Failure**: If the loop completes without finding a valid path, returns `StaleFile`.

**Edge Cases:**
- **Stale Handles**: If a file is deleted but the handle ID is still referenced, the metadata check will fail for all paths, resulting in `StaleFile`.
- **Hardlinks**: If multiple hardlinks exist, the mechanism returns the first one found in the `BTreeSet` iteration order.

**Complexity:**
- Time: O(N) where N is the number of hardlinks (paths) associated with the object key.
- Space: O(1).

**Determinism:**
- Deterministic relative to the file system state. The iteration order of `BTreeSet` is fixed, so the specific hardlink returned is consistent if multiple exist.

### Mechanism 3: Recursive Path Removal (`remove_path`)

**Intent:**
To maintain the consistency of internal mappings when a file or directory is deleted. It supports recursive removal, meaning deleting a directory automatically removes all tracked mappings for files within that directory without requiring explicit calls for each child.

**Inputs:**
- `path: &Path`: The absolute path of the file or directory being removed.

**Outputs:**
- None (modifies internal state in-place).

**Steps:**
1. **Normalization**: Strips the root prefix to get the relative path. Returns silently if the path is outside the root.
2. **Identification**: Collects all keys in `relative_to_key` where the stored relative path is equal to or starts with the input relative path.
3. **Removal**: Iterates over the identified relative paths:
   - Removes the entry from `relative_to_key`.
   - Removes the path from the `key_to_paths` set for the corresponding `ObjectKey`.
   - If the `key_to_paths` set becomes empty (no more hardlinks), it removes the entry from `key_to_paths`, `key_to_id`, and `id_to_key` (effectively freeing the handle ID).

**Edge Cases:**
- **Partial State**: If a directory is removed, but the internal map was missing some entries (e.g., due to prior errors), only the known entries are removed.
- **Non-existent Paths**: The method does not check if the path exists on disk; it operates purely on the internal map state.

**Complexity:**
- Time: O(M) where M is the number of tracked entries under the specified path.
- Space: O(L) where L is the number of entries being removed (temporary vector allocation).

**Determinism:**
- Deterministic.

### Mechanism 4: Recursive Path Renaming (`rename_path`)

**Intent:**
To update internal mappings to reflect a file or directory move operation. Like removal, this handles recursive updates, ensuring that moving a directory updates the internal relative paths of all contained files.

**Inputs:**
- `from: &Path`: The source absolute path.
- `to: &Path`: The destination absolute path.

**Outputs:**
- `Result<(), vfs::Error>`: Returns an error if the paths are invalid (e.g., outside root).

**Steps:**
1. **Normalization**: Strips the root prefix from both `from` and `to` to get relative paths.
2. **Identification**: Iterates over `relative_to_key` to find entries where the path is equal to or starts with the `from` relative path.
3. **Transformation**: For each matching entry:
   - Calculates the new relative path by replacing the `from` prefix with the `to` prefix.
   - Updates `relative_to_key` by removing the old key and inserting the new one.
   - Updates `key_to_paths` by removing the old path and inserting the new path for the associated `ObjectKey`.

**Edge Cases:**
- **Overwrites**: The logic assumes the destination does not currently exist in the map or that conflicts are handled by the caller/VFS layer before this method is called.
- **Prefix Matching**: Uses `starts_with` to correctly identify children of a moved directory.

**Complexity:**
- Time: O(M) where M is the number of tracked entries under the source path.
- Space: O(L) where L is the number of entries being moved (temporary vector allocation).

**Determinism:**
- Deterministic.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::file`**:
    - **`file::Handle`**: The module relies on this opaque wrapper for a byte array (`[u8; NFS3_FHSIZE]`) to store the encoded file identifiers. The `FsMap` encodes its internal `u64` IDs into this format using `encode_handle` and decodes them using `decode_handle`. This ensures that the identifiers passed to the VFS layer strictly adhere to the NFSv3 protocol size requirements.

- **From `nfs_mamont::vfs`**:
    - **`vfs::Error`**: The module uses this enum to classify failures. Specifically, it maps `std::io::ErrorKind::NotFound` to `vfs::Error::NoEntry` and generic I/O failures to `vfs::Error::IO`. It also uses `vfs::Error::StaleFile` to indicate that a handle no longer corresponds to any valid path on disk, and `vfs::Error::BadFileHandle` for invalid IDs.

---

## 4. Data Model

Entities:
- **`FsMap`**: The central mapping structure.
    - Fields: `root`, `next_id`, `id_to_key`, `key_to_id`, `key_to_paths`, `relative_to_key`.
- **`ObjectKey`**: A unique identifier for a filesystem object.
    - Fields: `dev: u64`, `ino: u64`.

Relations:
- **`FsMap` manages `u64` IDs**: `id_to_key` maps IDs to `ObjectKey`s.
- **`FsMap` manages `ObjectKey` aliases**: `key_to_paths` maps `ObjectKey`s to sets of relative paths (handling hardlinks).
- **`FsMap` manages path lookups**: `relative_to_key` maps relative paths to `ObjectKey`s for fast reverse lookup.

Global Invariants:
- **Root Handle**: The ID `1` is permanently reserved for the root directory and is never assigned to other objects.
- **ID Uniqueness**: Active IDs in `id_to_key` are unique and never `0`.
- **Consistency**: For every entry in `relative_to_key`, there must be a corresponding entry in `key_to_paths` containing that path, and vice versa.
- **Hardlink Integrity**: If multiple paths map to the same `ObjectKey`, they all resolve to the same ID.

## 5. Error Model

Error Types:
- **`vfs::Error`**: The primary error type returned by public methods.

Error Propagation Strategy:
- **Mapping**: The module uses a helper function `map_io_error` to convert `std::io::Error` into `vfs::Error`.
    - `NotFound` -> `NoEntry`
    - `PermissionDenied` -> `Access`
    - `InvalidInput` / `InvalidData` -> `InvalidArgument`
    - Others -> `IO`
- **Direct Usage**: `StaleFile` and `BadFileHandle` are returned directly when internal consistency checks fail (e.g., ID not found, path not found on disk).

Recoverability:
- **`StaleFile`**: Indicates the handle is invalid. The client must perform a new lookup to obtain a valid handle. The internal state remains unchanged (lazy cleanup).
- **`NoEntry`**: Indicates the path does not exist. The caller should not retry the same operation.
- **`IO`**: Indicates a transient or permanent disk error. Retriability depends on the specific cause.

Panics:
- **Allowed**: No.
- **Conditions**: The code avoids panics by using `?` operators and explicit checks (e.g., `ok_or`).

---

## 6. Traits

List which external traits this module implements:
- **`Debug`**: Implemented for `FsMap` and `ObjectKey`.

---

## 7. Overview

This module is used in order to **bridge the gap between the stateless, handle-based NFSv3 protocol and the stateful, path-based local filesystem**. The system contains a complex NFS server (`nfs_mamont`) that interacts with a local storage backend (mirror). The VFS layer expects to operate on opaque `file::Handle` identifiers, while the operating system requires file paths to perform I/O. This module is necessary because it maintains the dynamic mapping table that allows the server to translate between these two domains, ensuring that handles remain stable across operations and that hardlinks are handled correctly (i.e., multiple paths resolve to a single handle).

A typical usage scenario of the system involves a client requesting a `LOOKUP` operation for a file named "data.txt". The VFS layer calls `ensure_handle_for_path("/mnt/mirror/data.txt")`. The module checks the file's inode, finds it is new, allocates ID 5, and returns a `Handle` containing the bytes of 5. Later, the client requests a `READ` using that handle. The VFS layer calls `path_for_handle(&handle)`. The module decodes ID 5, looks up the inode, retrieves the stored path "/mnt/mirror/data.txt", verifies it still exists on disk, and returns the path to the backend to open the file.

Inside the system, the following things happen and they use this module:
1.  **Identity Abstraction**: The module uses `(dev, ino)` tuples (`ObjectKey`) as the canonical source of truth for file identity. This decouples the handle from the specific path used to access the file, which is essential for supporting hardlinks and renames without invalidating client handles.
2.  **State Synchronization**: When a file is renamed or deleted via the VFS, the `rename_path` and `remove_path` methods are called. These methods update the internal maps to reflect the new filesystem topology. Crucially, they operate recursively (e.g., renaming a directory updates all tracked children), ensuring that the internal state remains consistent with the disk without requiring a full scan.
3.  **Stale Handle Detection**: The `path_for_handle` method includes a verification step (`symlink_metadata`) before returning a path. This allows the system to detect if a file was deleted outside of the NFS protocol (or if a race condition occurred) and return `StaleFile` to the client, triggering a recovery sequence.

The critical aspect of this module is the **maintenance of bidirectional indices** (`id_to_key`, `key_to_paths`, `relative_to_key`). This design allows O(1) lookups in both directions (handle-to-path and path-to-handle) while supporting N-to-1 relationships (paths-to-inode) required for hardlinks. Without this module, the VFS implementation would have to rely on unstable paths or inefficient scanning to resolve file handles.

**Uncertainty**: The module uses `symlink_metadata` in `object_key_for_path` and `path_for_handle`. This implies it follows symlinks to get the inode but does not explicitly resolve the symlink target itself for the returned path. If a symlink is deleted and recreated, the handle might become stale or point to a new file depending on the inode reuse policy of the underlying filesystem, which is outside the control of this module.