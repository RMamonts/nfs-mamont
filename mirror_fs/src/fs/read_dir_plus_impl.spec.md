<!-- SPEC_HASH: 608e46e6842ef7643c8eb66a30c8ec30208cc9766e2ee26db118cfff970e2834 -->
# Module Specification

Module: mirrorfs::fs::read_dir_plus_impl
Rust File: mirror_fs/src/fs/read_dir_plus_impl.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`nfs_mamont::consts::nfsv3`**: Used to import `NFS3_WRITEVERFSIZE`. This constant is used in the size estimation logic for directory entries to ensure the response size does not exceed the client's `max_count` limit.
- **`nfs_mamont::vfs`**: Used to import the `vfs::Error` enum. This is used to wrap and return specific error conditions (such as `BadCookie` or `IO` errors) within the `read_dir_plus::Fail` structure.
- **`nfs_mamont::vfs::read_dir`**: Used to import the `read_dir::Cookie` type. This type is used to interpret the starting offset provided by the client and to generate the cookie for the next entry in the response.
- **`nfs_mamont::vfs::read_dir_plus`**: Used to import the `ReadDirPlus` trait and its associated types (`Args`, `Success`, `Fail`, `Entry`). This module provides the concrete implementation of this trait for the `MirrorFS` struct.
- **`super::MirrorFS`**: The struct for which the `ReadDirPlus` trait is being implemented. The implementation relies on methods of `MirrorFS` (e.g., `path_for_handle`, `list_directory_entries`, `handle_for_path`) to interact with the underlying file system.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

### Mechanism 1: Cookie Verifier Validation

**Intent:**
To ensure that the directory has not been modified between paginated `READDIRPLUS` requests. This prevents the client from receiving a corrupted or inconsistent view of the directory contents.

**Inputs:**
- `args.cookie`: The offset provided by the client.
- `args.cookie_verifier`: The verifier provided by the client from the previous response.
- Current directory metadata (retrieved via `Self::metadata`).

**Outputs:**
- `Ok(())` if the verifier matches or if the cookie is zero (start of read).
- `Err(read_dir_plus::Fail)` with `vfs::Error::BadCookie` if the verifier mismatches.

**Steps:**
1. Retrieve the metadata for the target directory using the handle provided in `args.dir`.
2. Generate a `cookie_verifier` from the current directory attributes.
3. If `args.cookie` is not zero, compare `args.cookie_verifier` with the generated verifier.
4. If they differ, return a `Fail` structure containing `vfs::Error::BadCookie`.

**Edge Cases:**
- **Initial Read**: If `args.cookie` is zero, the verifier check is skipped, allowing the start of a new read sequence.

**Complexity:**
- Time: O(1) for metadata retrieval and comparison (assuming metadata access is constant time).
- Space: O(1).

**Determinism:**
- Deterministic.

### Mechanism 2: Size-Constrained Entry Streaming

**Intent:**
To populate the response with directory entries while strictly adhering to the `max_count` limit specified by the client. This prevents the server from sending responses that exceed the client's buffer capacity or network MTU.

**Inputs:**
- `args.max_count`: The maximum byte size allowed for the response.
- `entries`: A list of directory entries (name, path, metadata) obtained from the file system.

**Outputs:**
- A `Vec<read_dir_plus::Entry>` containing as many entries as possible without exceeding `max_count`.

**Steps:**
1. Determine the starting index in the entry list using `args.cookie.raw()`.
2. Iterate through the entries starting from the calculated index.
3. For each entry, estimate its size in the response using the formula: `48 + name_length + NFS3_WRITEVERFSIZE`.
4. Check if adding this entry would cause the total accumulated size (`used`) to exceed `args.max_count`.
5. If the limit would be exceeded, stop iterating and break the loop.
6. Otherwise, add the entry to the result vector and update the `used` size.

**Edge Cases:**
- **Empty Result**: If the first entry exceeds `max_count`, the result vector may be empty, but `eof` might be false if there are more entries.
- **Zero Max Count**: If `max_count` is very small, the loop might break immediately.

**Complexity:**
- Time: O(N) where N is the number of entries scanned until the size limit is reached.
- Space: O(M) where M is the number of entries fitting in `max_count`.

**Determinism:**
- Deterministic.

### Mechanism 3: Attribute and Handle Resolution

**Intent:**
To fulfill the "Plus" requirement of the `READDIRPLUS` procedure by retrieving and returning full file attributes and file handles for each entry in the directory listing. This optimizes client performance by eliminating the need for subsequent `LOOKUP` calls.

**Inputs:**
- `path`: The file system path of a directory entry.
- `meta`: The metadata of the directory entry.

**Outputs:**
- `file::Attr`: The attributes derived from the metadata.
- `file::Handle`: The file handle generated from the path.

**Steps:**
1. Convert the raw metadata (`meta`) into a `file::Attr` structure using `Self::attr_from_metadata`.
2. Generate a `file::Handle` for the entry's path using `self.handle_for_path`.
3. If `handle_for_path` fails, the entire operation fails immediately, returning a `Fail` structure with the specific error.

**Edge Cases:**
- **Handle Generation Failure**: If a handle cannot be generated for a specific entry (e.g., due to path issues), the whole request fails, and no partial data is returned for that page.

**Complexity:**
- Time: O(1) per entry (assuming handle generation and attribute conversion are constant time).
- Space: O(1) per entry.

**Determinism:**
- Deterministic.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::read_dir_plus`**:
 - **`Args` and `max_count`**: This module relies on the `max_count` field in `Args` to enforce the size constraint mechanism. The implementation logic is driven by the need to keep the serialized `Success` structure within this byte limit.
 - **`Entry`**: The implementation constructs this structure for every file found, populating `file_attr` and `file_handle` as required by the trait definition.
 - **`Fail`**: Used to wrap errors, specifically utilizing the `dir_attr` field to return directory attributes even when the operation fails (e.g., due to `BadCookie`).

- **From `nfs_mamont::vfs::read_dir`**:
 - **`Cookie`**: The implementation uses `Cookie::raw()` to determine the starting index for iteration and `Cookie::new()` to assign the next cookie to each returned entry.

- **From `nfs_mamont::vfs`**:
 - **`vfs::Error::BadCookie`**: This specific error variant is critical for the validation mechanism. The implementation returns this error when the directory's modification state (inferred from attributes) does not match the client's verifier.

- **From `nfs_mamont::consts::nfsv3`**:
 - **`NFS3_WRITEVERFSIZE`**: This constant is used in the size estimation formula (`48 + name_len + NFS3_WRITEVERFSIZE`). It ensures that the estimation accounts for the protocol-specific size of the verifier field included in the response.

---

## 4. Data Model

Entities:
- **`read_dir_plus::Entry`**: Constructed for each valid directory entry returned.
 - Fields populated: `file_id`, `file_name`, `cookie`, `file_attr` (always `Some`), `file_handle` (always `Some`).
- **`read_dir_plus::Success`**: Returned on successful operation.
 - Fields populated: `dir_attr` (always `Some`), `cookie_verifier`, `entries`, `eof`.
- **`read_dir_plus::Fail`**: Returned on error.
 - Fields populated: `error`, `dir_attr` (populated if metadata retrieval succeeded before the error).

Relations:
- **Aggregation**: `Success` contains a vector of `Entry` entities.
- **Dependency**: The construction of `Entry` depends on successful resolution of `file::Attr` and `file::Handle` from the underlying `MirrorFS` storage.

Global Invariants:
- **Cookie Monotonicity**: The `cookie` assigned to an entry corresponds to its index + 1 in the full directory listing.
- **Size Limit**: The total estimated size of the returned entries will not exceed `args.max_count`.

## 5. Error Model

Error Types:
- **`vfs::Error`**: The primary error type returned within `read_dir_plus::Fail`.
 - **`BadCookie`**: Returned when the provided `cookie_verifier` does not match the current directory state.
 - **`IO` / `StaleFile` / etc.**: Propagated from underlying `MirrorFS` operations (`path_for_handle`, `metadata`, `list_directory_entries`, `handle_for_path`).

Error Propagation Strategy:
- **Early Return**: The function uses the `?` operator and explicit `match` statements to return immediately upon encountering an error from any underlying `MirrorFS` method or validation step.
- **Context Preservation**: When returning `Fail`, the module attempts to include `dir_attr` if it was successfully retrieved before the failure occurred, adhering to the NFSv3 specification.

Recoverability:
- **`BadCookie`**: Recoverable by the client restarting the read operation from the beginning (cookie = 0).
- **`IO`**: Recoverability depends on the nature of the I/O error (transient vs. permanent).
- **Handle Generation Failure**: If `handle_for_path` fails for an entry, the request fails entirely. The client may need to retry or skip the specific entry if the protocol allowed partial failure (which it generally doesn't for `READDIRPLUS`).

Panics:
- **Allowed**: No.
- **Conditions**: The code uses standard error handling (`Result`) and does not contain explicit `panic!` calls or `unwrap()` calls that would cause a crash under normal error conditions.

---

## 6. Traits

List which external traits this module implements:
- **`nfs_mamont::vfs::read_dir_plus::ReadDirPlus`**: Implements the `async fn read_dir_plus` method for the `MirrorFS` struct.

---

## 7. Overview

This module is used in order to **provide a concrete implementation of the NFSv3 `READDIRPLUS` procedure for the `MirrorFS` storage backend**. The system contains a modular NFS server architecture where the protocol logic (`nfs_mamont`) is separated from the storage implementation (`mirror_fs`). The `MirrorFS` struct acts as a bridge between the abstract VFS interface and the actual file system operations (reading directories, resolving paths, generating handles).

This module is necessary because the `READDIRPLUS` operation is significantly more complex than a standard directory listing. It requires the server to return not just filenames, but also full file attributes and file handles for every entry, all while strictly managing the response size to fit within the client's `max_count` buffer limit. Furthermore, it must implement state validation using `cookie_verifier` to ensure that the directory has not changed during a paginated read.

A typical usage scenario of the system involves a client executing a command like `ls -l` on a mounted NFS share. The client sends a `READDIRPLUS` request. The server dispatches this to the `MirrorFS` implementation. This module then resolves the directory handle to a local path, reads the directory contents, and iterates through the files. For each file, it generates a file handle and retrieves attributes, estimating the size of the data to ensure it doesn't overflow the client's buffer. If the directory changes mid-operation, the verifier check fails, and the client is forced to restart, ensuring data consistency.

Inside the system, the following things happen and they use this module:
1. **Protocol Enforcement**: The module enforces the `max_count` constraint defined in the NFSv3 RFC. By calculating the estimated size of each entry (including overhead, name length, and verifier size), it ensures the server does not send a packet that the client cannot handle.
2. **State Consistency**: The module implements the `cookie_verifier` check. By deriving the verifier from the directory's metadata, it ensures that if the directory is modified (e.g., a file is added or deleted) between requests from the client, the server rejects the subsequent request with `BadCookie`. This prevents the client from seeing a "torn" view of the directory where entries might be missing or duplicated.
3. **Data Enrichment**: The module fulfills the "Plus" promise of the procedure by calling `handle_for_path` and `attr_from_metadata` for every entry. This shifts the load of looking up file attributes from the client (which would otherwise need to make a `LOOKUP` call for every file) to the server, significantly improving performance for directory listings.

**Uncertainty**: The code references methods `Self::metadata`, `Self::attr_from_metadata`, `Self::validate_directory`, `Self::cookie_verifier_for_attr`, and `self.list_directory_entries`. These methods are not listed in the provided `*.facts.json` for `MirrorFS`. It is assumed that these are internal helper methods or methods defined in a private block/impl within `MirrorFS` that handle the specific logic of interacting with the local file system. Without their definitions, the exact logic for metadata retrieval and verifier generation is inferred from their usage context.