<!-- SPEC_HASH: dcaa75e062b92ea8c10831755aa45385acbddaea4ac6b8d0e4c5f38b46f8fb75 -->
# Module Specification

Module: mirrorfs::fs::read_dir_impl
Rust File: mirror_fs/src/fs/read_dir_impl.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`nfs_mamont::vfs::read_dir`**: Used to import the `ReadDir` trait and its associated types (`Args`, `Success`, `Fail`, `Entry`, `Cookie`, `CookieVerifier`). This module provides the concrete implementation of this trait for `MirrorFS`, defining how the NFSv3 `READDIR` procedure is executed against the local file system.
- **`nfs_mamont::vfs`**: Used to import the `vfs::Error` enum (specifically the `BadCookie` variant) and core file system types (`Handle`, `Name`, `Attr`). These are used to construct the response structures and to signal protocol-level errors back to the client.
- **`super::MirrorFS`**: The parent struct for which this implementation is defined. The implementation relies on internal methods of `MirrorFS` (e.g., `path_for_handle`, `metadata`, `list_directory_entries`) to interact with the underlying storage. *Note: The public interface facts for `MirrorFS` do not list these methods, implying they are internal or private helpers used to abstract OS-level file system operations.*

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

### Mechanism 1: Handle Resolution and Directory Validation

**Intent:**
To translate the opaque NFS file handle into a concrete file system path and ensure that the target object exists and is a directory before attempting to read its contents.

**Inputs:**
- `args.dir`: The `file::Handle` provided by the client.

**Outputs:**
- `Result<PathBuf, vfs::Error>`: The resolved path, or an error if the handle is invalid.
- `file::Attr`: The attributes of the directory, used for the response and validation.

**Steps:**
1. Calls `self.path_for_handle(&args.dir)` to map the handle to a path. If this fails, returns `Fail` immediately.
2. Calls `Self::metadata(&dir_path)` to retrieve the file system metadata. If this fails, returns `Fail`.
3. Converts metadata to `file::Attr` using `Self::attr_from_metadata`.
4. Calls `Self::validate_directory(&dir_attr)` to ensure the target is actually a directory. If not, returns `Fail` with the attributes.

**Edge Cases:**
- **Invalid Handle**: If `path_for_handle` fails, `dir_attr` is `None` in the response.
- **Not a Directory**: If validation fails, `dir_attr` is `Some` (the attributes of the non-directory file) in the response.

**Complexity:**
- Time: O(1) for handle lookup (assuming hash map), O(1) for metadata (stat call).
- Space: O(1).

**Determinism:**
- Deterministic.

### Mechanism 2: Cookie Consistency Verification

**Intent:**
To enforce the NFSv3 Weak Cache Consistency model by verifying that the directory has not been modified since the client started reading it. If the directory changes, the pagination state (cookies) becomes invalid.

**Inputs:**
- `args.cookie`: The cursor provided by the client.
- `args.cookie_verifier`: The verifier provided by the client.
- `dir_attr`: The current attributes of the directory.

**Outputs:**
- `Result<(), vfs::Error>`: Success if the verifier matches, or `BadCookie` error if it does not.

**Steps:**
1. Generates a `verifier` from the current `dir_attr` using `Self::cookie_verifier_for_attr`.
2. Checks if `args.cookie` is zero. If it is, the verifier check is skipped (start of read).
3. If `args.cookie` is non-zero, compares `args.cookie_verifier` with the generated `verifier`.
4. If they differ, returns `Fail` with `vfs::Error::BadCookie`.

**Edge Cases:**
- **Initial Read**: A cookie of 0 is always accepted regardless of the verifier, allowing the client to restart the stream.

**Complexity:**
- Time: O(1).
- Space: O(1).

**Determinism:**
- Deterministic.

### Mechanism 3: Directory Enumeration with Size Budgeting

**Intent:**
To iterate through the directory entries starting at the requested offset (`cookie`), accumulating entries until the byte size of the response approaches the client's specified limit (`count`). This ensures the response fits within network buffers while maximizing throughput.

**Inputs:**
- `dir_path`: The path to the directory.
- `args.cookie`: The starting index (offset).
- `args.count`: The maximum byte size of the response.

**Outputs:**
- `Vec<read_dir::Entry>`: The list of entries to return.
- `bool`: A flag indicating if the end of the directory was reached (`eof`).

**Steps:**
1. Calls `self.list_directory_entries(&dir_path)` to get the full list of entries (name, path, metadata).
2. Calculates the starting index `start` from `args.cookie.raw()`.
3. Iterates through the entries using `enumerate().skip(start)`.
4. For each entry, estimates the XDR size: `24 + name_length`.
5. Checks if adding this entry would exceed `args.count`. If the buffer is not empty and the limit is exceeded, the loop breaks.
6. Constructs a `read_dir::Entry` with:
   - `file_id`: Derived from metadata.
   - `file_name`: The entry name.
   - `cookie`: The index of the *next* entry (`index + 1`).
7. *Side Effect*: Calls `self.handle_for_path(&path).await` for each entry, ignoring the result. This likely serves to warm up a handle cache or ensure handles are generated, though it does not affect the return value.
8. Calculates `eof`: true if `start` is beyond the list length or if the loop processed all remaining entries.

**Edge Cases:**
- **Empty Directory**: Returns an empty list and `eof = true`.
- **Small Count**: If `count` is too small for even one entry, returns an empty list. `eof` depends on whether `start` is at the end.
- **Large Directory**: Only a slice of the directory is returned, determined by `count`.

**Complexity:**
- Time: O(N) to list the directory (where N is total entries) + O(M) to process the returned slice (where M is returned entries).
- Space: O(N) to hold the full entry list temporarily + O(M) for the result vector.

**Determinism:**
- Deterministic (assuming the underlying file system order is stable).

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::read_dir`**:
 - **`ReadDir` Trait**: This module implements the `read_dir` method defined by this trait. It adheres to the contract of taking `Args` and returning `Result<Success, Fail>`.
 - **`Cookie` and `CookieVerifier`**: The module uses these types to manage the pagination state. It relies on the `is_zero` method to check for initial requests and compares the raw bytes of the verifier.
 - **`Entry`**: The module constructs these structures, populating `file_id`, `file_name`, and the next `cookie` to be sent back to the client.

- **From `nfs_mamont::vfs`**:
 - **`vfs::Error::BadCookie`**: The module returns this specific error variant when the directory's modification state (inferred from attributes) does not match the client's verifier. This signals the client to restart the read operation.

- **From `mirrorfs::fs` (Assumed Internal Mechanics)**:
 - **`path_for_handle`**: Used to resolve the abstract `Handle` into a concrete `PathBuf`. This is the bridge between the NFS protocol identifiers and the local file system paths.
 - **`metadata`**: Used to retrieve `std::fs::Metadata` (or equivalent) to generate `file::Attr` and the cookie verifier.
 - **`list_directory_entries`**: Used to perform the actual directory scanning on the local disk, returning a list of names and paths.

---

## 4. Data Model

Entities:
- This module defines no new public structs or enums. It operates entirely on types defined in `nfs_mamont::vfs` and `nfs_mamont::vfs::read_dir`.

Relations:
- **Implementation**: `MirrorFS` implements `read_dir::ReadDir`.
- **Mapping**: The module maps `read_dir::Args` (handles, cookies) to local file system operations (paths, indices).

Global Invariants:
- **Cookie Semantics**: The `cookie` value returned in an entry corresponds to the 0-based index of that entry in the directory listing plus 1. A request with cookie `K` will skip the first `K` entries.
- **Verifier Generation**: The `cookie_verifier` is derived deterministically from the directory's attributes (likely `mtime` or `inode` changes). If the directory changes, the verifier changes.

## 5. Error Model

Error Types:
- **`vfs::Error`**: Wrapped inside `read_dir::Fail`.
 - **`BadCookie`**: Returned when the provided `cookie_verifier` does not match the current directory state.
 - **Other Errors (IO, NotDir, etc.)**: Propagated from internal methods like `path_for_handle`, `metadata`, or `list_directory_entries`.

Error Propagation Strategy:
- **Early Return**: The function uses a series of `match` statements. If any step (handle resolution, metadata fetching, validation, listing) fails, it immediately returns a `read_dir::Fail` struct containing the error and the `dir_attr` (if available).

Recoverability:
- **`BadCookie`**: Recoverable. The client should restart the read with `cookie=0` and `cookie_verifier=0`.
- **`IO` / `StaleFile`**: Generally not recoverable within the same request; the client must re-lookup the directory or handle the I/O failure.

Panics:
- **Allowed**: No.
- **Conditions**: The code uses `saturating_add` for size calculations to prevent overflow panics. All error paths are handled via `Result`.

---

## 6. Traits

List which external traits this module implements:
- **`nfs_mamont::vfs::read_dir::ReadDir`**: Implemented for `MirrorFS`.

---

## 7. Overview

This module is used in order to **bridge the gap between the stateless, paginated NFSv3 directory listing protocol and the stateful, full-listing nature of standard operating system file systems**. The system contains an NFS server (`nfs_mamont`) that abstracts various storage backends behind a VFS interface. The `MirrorFS` backend is a specific implementation that mirrors a local directory tree.

This module is necessary because the NFS protocol requires directory reads to be chunked (to respect packet size limits defined by `Args::count`) and consistent (using `CookieVerifier` to detect changes). Standard OS APIs (like `readdir` or `std::fs::read_dir`) provide a sequential stream of entries but do not natively support "jump to offset N" or "verify directory hasn't changed" semantics efficiently.

A typical usage scenario of the system involves a client executing `ls` on a mounted directory. The client sends a `READDIR` request with `cookie=0`. This module resolves the handle, lists the directory, and fills the response buffer up to the `count` limit. It returns a `cookie` (e.g., 32) for the last entry. The client then sends a request with `cookie=32`. If another client modified the directory in the interim, the `cookie_verifier` calculated from the directory's current metadata will differ from the one the client holds. This module detects this mismatch and returns `BadCookie`, forcing the client to restart the listing to ensure it sees a consistent snapshot of the directory.

Inside the system, the following things happen and they use this module:
1.  **Protocol Translation**: The module translates the high-level NFS request into specific `MirrorFS` internal calls (`path_for_handle`, `list_directory_entries`).
2.  **Resource Management**: It calculates the byte size of the XDR-encoded response (`24 + name length`) to ensure the server does not generate responses larger than the client can handle, preventing network fragmentation or errors.
3.  **State Synchronization**: By calling `handle_for_path` for every entry in the result set (and ignoring the result), the module likely triggers the pre-generation or caching of file handles for the visible files, optimizing subsequent `LOOKUP` operations from the client.

The critical aspect of this module is the **implementation of the cookie logic**. It treats the directory listing as a 0-indexed array. The cookie is simply the index + 1. This allows the server to efficiently skip entries using `skip(start)` on the iterator, satisfying the NFS requirement for resuming reads without maintaining server-side state for every active directory read.