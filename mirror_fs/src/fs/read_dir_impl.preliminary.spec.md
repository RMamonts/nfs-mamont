<!-- SPEC_HASH: dcaa75e062b92ea8c10831755aa45385acbddaea4ac6b8d0e4c5f38b46f8fb75 -->
# Module Specification

Module: mirrorfs::fs::read_dir_impl
Rust File: mirror_fs/src/fs/read_dir_impl.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`nfs_mamont::vfs::read_dir`**: Used to import the `ReadDir` trait and its associated types (`Args`, `Success`, `Fail`, `Entry`, `Cookie`, `CookieVerifier`). This module provides the concrete implementation of the `read_dir` asynchronous method for the `MirrorFS` struct, enabling it to serve NFSv3 `READDIR` requests.
- **`nfs_mamont::vfs`**: Used to access the `vfs::Error` enum (specifically `vfs::Error::BadCookie`) which is returned when the directory state has changed between requests, invalidating the client's cookie.
- **`super::MirrorFS`**: The struct for which the `ReadDir` trait is being implemented. The implementation relies on several internal (non-public) methods of `MirrorFS`—assumed to be `path_for_handle`, `metadata`, `attr_from_metadata`, `validate_directory`, `cookie_verifier_for_attr`, and `list_directory_entries`—to perform the underlying file system operations.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

### Mechanism 1: `READDIR` Implementation for `MirrorFS`

**Intent:**
To fulfill the NFSv3 `READDIR` contract for the `MirrorFS` backend. This involves translating opaque file handles into filesystem paths, validating directory state, iterating over directory entries, and respecting client-side buffer limits (`count`) and pagination state (`cookie`).

**Inputs:**
- `args: read_dir::Args`: Contains the directory `Handle`, the current `Cookie` (offset), the `CookieVerifier` (consistency check), and the maximum byte `count` for the response.

**Outputs:**
- `Result<read_dir::Success, read_dir::Fail>`: Returns a structure containing the directory entries, updated cookie/verifier, and EOF status, or an error with optional directory attributes.

**Steps:**
1. **Path Resolution**: Calls `self.path_for_handle(&args.dir).await` to convert the NFS file handle into a local filesystem path. If this fails, returns `Fail` with `dir_attr: None`.
2. **Metadata Retrieval**: Calls `Self::metadata(&dir_path)` to obtain the directory's metadata. If this fails, returns `Fail` with `dir_attr: None`.
3. **Attribute Conversion**: Converts the raw metadata into `Attr` using `Self::attr_from_metadata`.
4. **Directory Validation**: Calls `Self::validate_directory(&dir_attr)`. If validation fails (e.g., not a directory), returns `Fail` with `dir_attr: Some(dir_attr)`.
5. **Verifier Generation**: Calculates the current `CookieVerifier` using `Self::cookie_verifier_for_attr(&dir_attr)`.
6. **Cookie Verification**:
   - If the client's `cookie` is non-zero, the client's `cookie_verifier` must match the generated `verifier`.
   - If they do not match, returns `Fail` with `vfs::Error::BadCookie` and `dir_attr: Some(dir_attr)`.
7. **Entry Listing**: Calls `self.list_directory_entries(&dir_path)` to get a vector of `(name, path, meta)` tuples. If this fails, returns `Fail` with `dir_attr: Some(dir_attr)`.
8. **Iteration and Filtering**:
   - Calculates the starting index `start` from `args.cookie.raw()`.
   - Iterates through the entries starting at `start`.
   - For each entry, estimates the XDR size as `(24 + name.as_str().len()) as u32`.
   - Checks if adding this entry exceeds `args.count`. If the buffer is full (and not empty), the loop breaks.
   - **Side Effect**: Calls `self.handle_for_path(&path).await` (ignoring the result), likely to warm up a handle cache.
   - Constructs a `read_dir::Entry` with `file_id`, `file_name`, and a new `cookie` (current index + 1).
9. **EOF Determination**: Sets `eof` to `true` if the start index was past the end or if the last entry added was the final entry in the directory.
10. **Return**: Returns `Success` with the collected entries, the verifier, and the EOF flag.

**Edge Cases:**
- **Initial Request**: If `args.cookie` is zero, the `cookie_verifier` check is skipped.
- **Empty Directory**: Returns `Success` with an empty entry list and `eof: true`.
- **Buffer Overflow**: If a single entry exceeds `args.count`, the loop logic implies it might not be added if `result` is not empty, or if the estimation logic prevents it. However, the code checks `!result.is_empty()` before breaking, ensuring at least one entry might be returned if it fits, or strictly enforcing the limit if the buffer is already partially filled.
- **Handle Cache Miss**: The call to `handle_for_path` is explicitly ignored (`let _`), indicating that failure to generate a handle for an entry does not fail the `READDIR` operation itself.

**Complexity:**
- **Time**: O(N) where N is the number of entries in the directory. The implementation iterates from the start index to the end of the list (or until the buffer is full).
- **Space**: O(M) where M is the number of entries returned, bounded by `args.count`.

**Determinism:**
- **Deterministic**: Given the same filesystem state and arguments, the output is consistent.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::read_dir`**:
    - **`ReadDir` Trait**: The module implements this trait, specifically the `read_dir` method. It adheres to the contract of returning `Result<Success, Fail>`.
    - **`Cookie` and `CookieVerifier`**: The module uses `cookie.is_zero()` to determine if the request is the first in a sequence. It compares `cookie_verifier` values to detect directory modifications.
    - **`Args::count`**: The module uses this field to limit the size of the response. It employs a heuristic size calculation (`24 + name length`) to approximate the XDR encoding size.

- **From `nfs_mamont::vfs`**:
    - **`vfs::Error::BadCookie`**: This specific error variant is returned when the provided `cookie_verifier` does not match the current directory state, signaling to the client that the directory has changed and the pagination is invalid.

---

## 4. Data Model

Entities:
- This module defines no public structs or enums. It operates entirely on types defined in `nfs_mamont::vfs::read_dir` and `nfs_mamont::vfs`.

Relations:
- **`MirrorFS` implements `ReadDir`**: This module provides the implementation logic connecting the `MirrorFS` internal state to the `ReadDir` interface.

Global Invariants:
- **Cookie Semantics**: The implementation treats the `cookie` as a 1-based index into the directory entry list (i.e., `cookie = index + 1`). A `cookie` of 0 implies reading from the start.
- **Verifier Consistency**: The `cookie_verifier` is derived from the directory's attributes. If the attributes change, the verifier changes, causing subsequent requests to fail with `BadCookie`.

---

## 5. Error Model

Error Types:
- **`vfs::Error`**: Wrapped inside `read_dir::Fail`.
    - `BadCookie`: Returned if the directory state changed between requests.
    - Other errors (e.g., `IO`, `NoEntry`): Propagated from internal methods like `path_for_handle` or `metadata`.

Error Propagation Strategy:
- **Early Return**: The function returns immediately upon encountering an error from internal operations (`path_for_handle`, `metadata`, `list_directory_entries`).
- **Attribute Preservation**: The `Fail` struct includes `dir_attr` (Option<file::Attr>). The module attempts to populate this field if the error occurred *after* metadata retrieval (e.g., during validation or listing), but leaves it as `None` if the error occurred during path resolution or initial metadata fetch.

Recoverability:
- **`BadCookie`**: Recoverable by the client restarting the read operation with `cookie = 0`.
- **`IO` / `NoEntry`**: Generally non-recoverable for the specific RPC call; the client must typically handle the failure (e.g., stop listing or retry the whole operation).

Panics:
- **Allowed**: Indirectly.
- **Conditions**: Panics may occur if the internal methods of `MirrorFS` (e.g., `metadata`, `list_directory_entries`) panic. The implementation itself uses `saturating_add` to prevent arithmetic panics on overflow.

---

## 6. Traits

List which external traits this module implements:
- **`nfs_mamont::vfs::read_dir::ReadDir`**: Implemented for `super::MirrorFS`.

---

## 7. Overview

This module is used in order to **provide the concrete implementation of the NFSv3 `READDIR` procedure for the `MirrorFS` backend**. The system contains a modular NFS server (`nfs_mamont`) that abstracts storage operations behind the `Vfs` trait. The `MirrorFS` is a specific storage backend (likely a wrapper around a local filesystem or a mirror of one) that must implement these operations to be usable by the server.

This module is necessary because the generic `ReadDir` trait only defines the *interface* (arguments, return values, and async behavior), not the *logic* of how to actually read a directory. The `MirrorFS` backend has specific internal representations (likely mapping NFS handles to file paths) that require custom logic to satisfy the `ReadDir` contract.

A typical usage scenario of the system involves a client requesting a directory listing. The server's RPC dispatcher calls the `read_dir` method on the `MirrorFS` instance. This implementation resolves the opaque handle to a path, checks if the directory has been modified (using the verifier), lists the contents, and then carefully packages the entries into a response that fits within the byte limit specified by the client. It also handles the "cookie" logic, allowing the client to request subsequent chunks of the directory listing.

Inside the system, the following things happen and they use this module:
1.  **Handle Translation**: The module relies on `path_for_handle` to bridge the gap between the NFS protocol's opaque handles and the backend's concrete paths.
2.  **State Validation**: By implementing the `cookie_verifier` check, the module ensures that the client does not receive a corrupted or inconsistent view of the directory if files are added or removed while the listing is in progress.
3.  **Handle Pre-caching**: The call to `handle_for_path` for every entry returned suggests an optimization strategy where the backend proactively generates or caches the file handles for the listed entries, anticipating that the client will likely perform `LOOKUP` or `STAT` operations on them immediately after receiving the directory listing.

**Assumptions**: The code references several methods on `MirrorFS` (e.g., `path_for_handle`, `metadata`, `list_directory_entries`, `cookie_verifier_for_attr`) that are not listed in the public interface of `MirrorFS` in the provided `*.facts.json`. It is assumed that these methods exist as `pub(crate)` or private methods within the `MirrorFS` implementation or its parent module, accessible to this `impl` block.