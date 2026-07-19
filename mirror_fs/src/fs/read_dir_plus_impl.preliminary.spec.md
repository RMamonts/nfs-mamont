<!-- SPEC_HASH: 608e46e6842ef7643c8eb66a30c8ec30208cc9766e2ee26db118cfff970e2834 -->
# Module Specification

Module: mirrorfs::fs::read_dir_plus_impl
Rust File: mirror_fs/src/fs/read_dir_plus_impl.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`nfs_mamont::consts::nfsv3`**: Used to import `NFS3_WRITEVERFSIZE`. This constant is utilized in the heuristic calculation of the estimated size of each directory entry to ensure the accumulated response size does not exceed the client's `max_count` limit.
- **`nfs_mamont::vfs`**: Used to import the `vfs::Error` enum (specifically `BadCookie`) and the `read_dir` module (for `Cookie`). These are essential for validating the state of the directory read operation and constructing the pagination cookies.
- **`nfs_mamont::vfs::read_dir_plus`**: Used to import the `ReadDirPlus` trait and its associated types (`Args`, `Success`, `Fail`, `Entry`). This module provides the concrete implementation of this trait for the `MirrorFS` struct.
- **`super::MirrorFS`**: The struct for which the `ReadDirPlus` trait is being implemented. The implementation relies on several methods of `MirrorFS` (e.g., `path_for_handle`, `metadata`, `list_directory_entries`, `handle_for_path`) to bridge the NFS protocol interface with the underlying file system operations.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

### Mechanism 1: Handle Resolution and Directory Validation

**Intent:**
To translate the opaque NFS file handle provided by the client into a concrete local filesystem path, verify that the path points to a valid directory, and ensure the directory has not been modified since the client's last request (using the cookie verifier).

**Inputs:**
- `args.dir`: The file handle provided by the client.
- `args.cookie`: The pagination offset.
- `args.cookie_verifier`: The verifier from the previous response.

**Outputs:**
- `dir_path`: The local filesystem path corresponding to the handle.
- `dir_attr`: The attributes of the directory.
- `Fail`: If resolution, validation, or consistency checks fail.

**Steps:**
1. Call `self.path_for_handle(&args.dir).await` to resolve the handle to a path. If this fails, return `Fail` with `dir_attr: None`.
2. Call `Self::metadata(&dir_path)` to retrieve the directory's metadata. If this fails, return `Fail` with `dir_attr: None`.
3. Convert metadata to `dir_attr` using `Self::attr_from_metadata`.
4. Call `Self::validate_directory(&dir_attr)` to ensure the target is a directory. If this fails, return `Fail` with `dir_attr: Some(dir_attr)`.
5. Generate a `verifier` using `Self::cookie_verifier_for_attr(&dir_attr)`.
6. If `args.cookie` is not zero and `args.cookie_verifier` does not match the generated `verifier`, return `Fail` with `vfs::Error::BadCookie` and `dir_attr: Some(dir_attr)`.

**Edge Cases:**
- **Stale Handle**: If `path_for_handle` fails, the operation aborts immediately without returning attributes.
- **Directory Modification**: If the directory changes between requests, the verifier mismatch triggers a `BadCookie` error, forcing the client to restart the read.

**Complexity:**
- Time: O(1) for resolution and validation (assuming handle lookup is constant time).
- Space: O(1).

**Determinism:**
- Deterministic.

### Mechanism 2: Entry Streaming with Size Budgeting

**Intent:**
To iterate through the directory entries starting from the requested cookie, constructing full `Entry` objects (including attributes and handles) while strictly adhering to the `max_count` size limit specified by the client to prevent buffer overflows.

**Inputs:**
- `dir_path`: The path to the directory.
- `args.max_count`: The maximum allowed size of the response structure.
- `args.cookie`: The starting index for iteration.

**Outputs:**
- `result`: A vector of `read_dir_plus::Entry` objects.
- `used`: The total accumulated size of the entries.

**Steps:**
1. Call `self.list_directory_entries(&dir_path)` to get a list of all entries (name, path, metadata). If this fails, return `Fail`.
2. Determine the starting index `start` from `args.cookie.raw()`.
3. Iterate through the entries starting at `start`.
4. For each entry, calculate the `estimated` size using the formula: `(48 + name.as_str().len() + NFS3_WRITEVERFSIZE) as u32`.
5. Check if `used.saturating_add(estimated) > args.max_count`. If true, stop the iteration.
6. Retrieve the file handle via `self.handle_for_path(&path).await`. If this fails, return `Fail` with the directory attributes.
7. Construct the `Entry` with `file_attr` and `file_handle` set to `Some`.
8. Update `used` with the estimated size and push the entry to `result`.

**Edge Cases:**
- **Empty Directory**: If `entries` is empty, the loop is skipped, and `eof` is set to true.
- **Large Entries**: If a single entry's estimated size exceeds the remaining `max_count`, it is excluded, and the loop terminates.
- **Handle Lookup Failure**: Failure to retrieve a handle for a specific entry aborts the entire operation.

**Complexity:**
- Time: O(N) where N is the number of entries in the directory (scanning is required to reach the start index) plus the number of entries returned.
- Space: O(M) where M is the number of entries returned, bounded by `max_count`.

**Determinism:**
- Deterministic.

### Mechanism 3: EOF Calculation

**Intent:**
To determine if the end of the directory has been reached based on the current iteration state and the number of entries returned.

**Inputs:**
- `start`: The starting index.
- `entries.len()`: The total number of entries in the directory.
- `result.len()`: The number of entries returned in this response.

**Outputs:**
- `eof`: A boolean flag indicating if the directory listing is complete.

**Steps:**
1. Check if `start >= entries.len()`. If true, set `eof = true`.
2. Otherwise, check if `start.saturating_add(result.len()) >= entries.len()`. If true, set `eof = true`.
3. Else, set `eof = false`.

**Edge Cases:**
- **Saturation**: Uses `saturating_add` to prevent arithmetic overflow, though unlikely with standard directory sizes.

**Complexity:**
- Time: O(1).
- Space: O(1).

**Determinism:**
- Deterministic.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::read_dir_plus`**:
 - **`Args` and `Success`**: The module consumes `Args` to drive the logic (specifically `max_count`, `cookie`, `cookie_verifier`) and produces a `Success` struct populated with `Entry` objects, `dir_attr`, and `eof`.
 - **`Entry`**: The module constructs these structs, ensuring that `file_attr` and `file_handle` are populated (wrapped in `Some`), fulfilling the promise of `READDIRPLUS` to return full information.

- **From `nfs_mamont::vfs::read_dir`**:
 - **`Cookie`**: The module uses the `raw()` method of the `Cookie` to determine the starting index for directory iteration and creates new `Cookie` instances for the returned entries.

- **From `nfs_mamont::vfs`**:
 - **`vfs::Error`**: The module propagates errors (like `IO`, `StaleFile`) from internal operations and explicitly generates `BadCookie` when the verifier check fails.

- **From `nfs_mamont::consts::nfsv3`**:
 - **`NFS3_WRITEVERFSIZE`**: This constant is used in the size estimation heuristic to account for the byte size of the verifier field in the response.

---

## 4. Data Model

Entities:
- **`read_dir_plus::Entry`**: Constructed within the loop.
 - Fields: `file_id` (from metadata), `file_name` (from listing), `cookie` (index + 1), `file_attr` (Some), `file_handle` (Some).
 - Invariants: `file_attr` and `file_handle` are always `Some` in this implementation.
- **`read_dir_plus::Success`**: Returned on success.
 - Fields: `dir_attr` (Some), `cookie_verifier` (calculated), `entries` (Vec), `eof` (calculated).
- **`read_dir_plus::Fail`**: Returned on failure.
 - Fields: `error` (vfs::Error), `dir_attr` (Option).

Relations:
- **Composition**: `Success` contains a vector of `Entry`.
- **Dependency**: The construction of `Entry` depends on successful `handle_for_path` and `metadata` lookups.

Global Invariants:
- **Size Limit**: The total size of the `entries` vector in bytes (estimated) will not exceed `args.max_count`.
- **Cookie Monotonicity**: The `cookie` value in each returned entry corresponds to the entry's index + 1, ensuring strictly increasing values within the response.

## 5. Error Model

Error Types:
- **`vfs::Error`**: The primary error type returned within `Fail`.
 - `BadCookie`: Explicitly returned if the directory's modification time (implied by the verifier) has changed.
 - `IO`, `StaleFile`, `NotDir`, etc.: Propagated from internal methods like `path_for_handle`, `metadata`, `list_directory_entries`, and `handle_for_path`.

Error Propagation Strategy:
- **Early Return**: The function uses the `?` operator or explicit `match` blocks to return immediately upon encountering an error from internal calls (`path_for_handle`, `metadata`, etc.).
- **Attribute Preservation**: If an error occurs after `dir_attr` is successfully retrieved (e.g., during `list_directory_entries` or `handle_for_path`), the error is returned with `dir_attr: Some(...)` to allow the client to cache the directory attributes despite the failure.

Recoverability:
- **`BadCookie`**: Recoverable by the client restarting the read from the beginning (cookie = 0).
- **`IO`**: Recoverability depends on the nature of the I/O error (transient vs. permanent).
- **`StaleFile`**: Recoverable by the client performing a new `LOOKUP` to obtain a fresh handle.

Panics:
- **Allowed**: No.
- **Conditions**: The code uses `saturating_add` to prevent overflow panics. All other error paths are handled via `Result`.

---

## 6. Traits

List which external traits this module implements:
- **`nfs_mamont::vfs::read_dir_plus::ReadDirPlus`**: Implements the `async fn read_dir_plus` method for the `MirrorFS` struct.

---

## 7. Overview

This module is used in order to **provide the concrete implementation of the NFSv3 `READDIRPLUS` procedure for the `MirrorFS` backend**. The system contains a modular NFS server architecture where the protocol logic (`nfs_mamont`) is separated from the storage backend implementation (`mirrorfs`). The `READDIRPLUS` procedure is distinct from standard directory listing because it requires the server to return not just filenames, but also file attributes and file handles for every entry in a single response. This is critical for client performance, as it eliminates the need for subsequent `LOOKUP` calls to retrieve metadata for each file.

The `MirrorFS` struct acts as a wrapper around a local filesystem (or a mirrored set of filesystems). This module is necessary because it translates the high-level NFS request (defined by `Args` and `Success` in `nfs_mamont::vfs::read_dir_plus`) into the specific low-level operations required by `MirrorFS` (resolving handles to paths, reading directories, looking up metadata, and generating handles).

A typical usage scenario of the system involves a client executing `ls -l` on a mounted directory. The client sends a `READDIRPLUS` request. The `MirrorFS` implementation in this module resolves the directory handle, reads the local file system entries, and iterates through them. For each entry, it calculates the NFS attributes, generates a file handle, and checks if adding this entry to the response would exceed the client's `max_count` buffer size. If the limit is reached, it stops and returns the partial list, setting `eof` to false. If the directory has changed since the last read (detected via the cookie verifier), it returns a `BadCookie` error.

Inside the system, the following things happen and they use this module:
1.  **Protocol Enforcement**: The module ensures that the `MirrorFS` backend adheres to the `ReadDirPlus` trait contract, specifically the requirement to return `Entry` objects populated with attributes and handles.
2.  **Resource Management**: By implementing the size estimation logic (`48 + name_len + NFS3_WRITEVERFSIZE`), the module ensures that the backend respects the client's memory constraints, preventing the generation of responses that are too large for the client to receive or process.
3.  **State Consistency**: The module implements the cookie verifier check, which is crucial for maintaining a consistent view of the directory in a concurrent environment where other processes might be modifying the directory contents.

**Uncertainty**: The provided `facts.json` for `MirrorFS` lists only `new`, `root_handle`, and `handle_for_path` as public methods. However, the code in this module relies on `path_for_handle`, `metadata`, `validate_directory`, `cookie_verifier_for_attr`, `list_directory_entries`, and `attr_from_metadata`. The specification assumes these methods exist on `MirrorFS` or its super-traits, as they are invoked as `self.method(...)` or `Self::method(...)`.