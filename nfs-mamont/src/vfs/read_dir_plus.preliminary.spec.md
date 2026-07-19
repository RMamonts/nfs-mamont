<!-- SPEC_HASH: 0b0a77f5fc86f46f794300dc5428d49ff404c4acdf7a36dfbb201c6224a55c39 -->
# Module Specification

Module: nfs_mamont::vfs::read_dir_plus
Rust File: src/vfs/read_dir_plus.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **crate::vfs**: Used to import the `Error` enum. This is required to define the `Fail` struct, which wraps a `vfs::Error` to indicate the specific reason for the operation's failure (e.g., `BadCookie`, `IO`).
- **crate::vfs::read_dir**: Used to import the `Cookie` and `CookieVerifier` types. These types are essential for maintaining the stateless resumption of directory reads, consistent with the standard `READDIR` operation.
- **super::file**: Used to import the `Handle`, `Name`, and `Attr` types. These are used to identify the directory being read (`Handle`), name the entries (`Name`), and provide the rich metadata (`Attr`) and unique identifiers (`Handle`) for each entry that distinguish `READDIRPLUS` from standard `READDIR`.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define the interface and data structures for the NFSv3 `READDIRPLUS` procedure.
- To optimize directory listing performance by allowing the server to return file attributes and file handles alongside file names and cookies in a single response. This eliminates the need for the client to perform subsequent `LOOKUP` or `GETATTR` RPCs for every entry in the directory.
- To enforce the specific size constraints of the NFSv3 protocol regarding the separation of "directory information" size (`dir_count`) and total response size (`max_count`).

Inputs:
- `Args`: A structure containing the directory `file::Handle`, a `Cookie` (offset), a `CookieVerifier` (state check), `dir_count` (maximum bytes for names/cookies), and `max_count` (maximum bytes for the entire response).

Outputs:
- `Result<Success, Fail>`:
 - `Success`: Contains the directory attributes, a new `CookieVerifier`, a vector of `Entry` objects (which include attributes and handles), and an `eof` flag.
 - `Fail`: Contains a `vfs::Error` and optional directory attributes.

Steps:
1. **Argument Processing**: The `read_dir_plus` method accepts `Args`. The implementation must interpret `dir_count` as the limit for the "directory information" (file names and cookies) and `max_count` as the limit for the entire structure, including attributes and file handles.
2. **State Validation**: The implementation must verify the `cookie_verifier` against the current state of the directory. If the directory has changed since the cookie was issued, the operation must fail with `vfs::Error::BadCookie`.
3. **Entry Retrieval**: Starting from the offset indicated by `cookie`, the implementation reads directory entries.
4. **Data Enrichment**: For each entry read, the implementation attempts to retrieve the `file::Attr` (attributes) and `file::Handle` (file handle). These are stored in the `Entry` struct.
5. **Size Management**: The implementation accumulates entries. It must stop if:
 - The size of the names and cookies exceeds `dir_count`.
 - The total size of the response (including XDR overhead, attributes, and handles) exceeds `max_count`.
6. **Response Construction**: The method returns `Success` with the accumulated entries, a new `cookie_verifier` for the next request, and `eof` set to `true` if the end of the directory was reached.

Edge Cases:
- **Partial Information**: The `Entry` struct defines `file_attr` and `file_handle` as `Option`. This implies that if the server cannot retrieve attributes or a handle for a specific entry (e.g., due to permissions or internal errors), it may still return the entry with these fields set to `None` rather than failing the entire request.
- **Size Constraints**: The dual constraint (`dir_count` vs `max_count`) is specific to `READDIRPLUS`. It is possible to fit within `max_count` but exceed `dir_count` (if attributes are small but names are long), or vice-versa. The implementation must respect the stricter of the two constraints at any given moment.

Complexity:
- Time: O(N) where N is the number of entries read. Retrieving attributes and handles for each entry may involve additional lookups, potentially increasing the constant factor compared to standard `READDIR`.
- Space: O(N) for the `Vec<Entry>` in the `Success` struct.

Determinism:
- Deterministic (assuming the underlying file system state remains consistent).

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::read_dir`**:
 - **State Management**: This module reuses the `Cookie` and `CookieVerifier` mechanics. The logic that a `Cookie` of 0 implies the start of the directory and that a mismatched `CookieVerifier` implies a `BadCookie` is inherited from the `read_dir` module's specification.
 - **Iteration Logic**: The concept of the `cookie` pointing to the *next* entry (as seen in `read_dir::Entry`) is preserved here, allowing the client to resume the listing.
- **From `nfs_mamont::vfs::file`**:
 - **Type Safety**: The module relies on the `Handle`, `Name`, and `Attr` types to ensure that the data returned in `Entry` is validated and conforms to the VFS layer's constraints (e.g., name length limits).
 - **Attribute Payload**: The `Attr` struct provides the full metadata (mode, uid, size, etc.) that `READDIRPLUS` promises to deliver, enabling the client to populate its attribute cache ("acache") without further network calls.

---

## 4. Data Model

Entities:
- **Entry**: Represents a file system object within a directory. It extends the standard directory entry by including `file_attr` (metadata) and `file_handle` (unique identifier). It also contains `file_id`, `file_name`, and `cookie`.
- **Args**: Arguments for the `read_dir_plus` operation. Distinct from `read_dir::Args` by the inclusion of both `dir_count` and `max_count`.
- **Success**: The successful result, containing directory attributes, a verifier, a list of `Entry` objects, and an EOF flag.
- **Fail**: The failure result, wrapping a `vfs::Error` and optional directory attributes.

Relations:
- **Composition**: `Entry` aggregates `file::Attr` and `file::Handle` optionally.
- **Composition**: `Args` aggregates `read_dir::Cookie` and `read_dir::CookieVerifier`.
- **Association**: `Fail` is associated with `vfs::Error`.

Global Invariants:
- **Zero File ID**: As per the documentation comment, `file_id` in `Entry` should not be 0 to avoid conflicts with UNIX semantics where 0 often has special meaning.
- **Size Semantics**: `dir_count` strictly refers to the byte count of the file names and cookies, excluding the attributes and file handles.

## 5. Error Model

Error Types:
- `vfs::Error` (specifically `vfs::Error::BadCookie` is relevant for state mismatches).

Error Propagation Strategy:
- The `read_dir_plus` function returns a `Result<Success, Fail>`. Errors are wrapped in the `Fail` struct, which includes the specific `vfs::Error` and optionally the directory attributes (`dir_attr`) to support Weak Cache Consistency (WCC) data on failure.

Recoverability:
- Recoverable. If `BadCookie` is returned, the client must restart the directory read from the beginning (cookie 0). Other errors imply standard file system error handling.

Panics:
- Allowed: No.
- Conditions: This module defines data structures and a trait interface. It contains no executable logic that could result in a panic.

---

## 6. Traits

List which external traits this module implements:
- **Send**: The `ReadDirPlus` trait is transformed into a Send-safe object trait via the `#[trait_variant::make(Send)]` attribute macro, allowing it to be passed between threads.

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependencies. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here you MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to define the contract for the `READDIRPLUS` operation in the `nfs_mamont` NFSv3 server. The system contains a Virtual File System (VFS) layer that abstracts storage operations. While the standard `READDIR` operation (defined in the `read_dir` module) allows clients to list directory contents, it only provides names and file IDs. In a typical usage scenario, a client performing an operation like `ls -l` needs file attributes (permissions, owner, size) and file handles for every entry. Without `READDIRPLUS`, the client would be forced to issue a separate `LOOKUP` (and potentially `GETATTR`) RPC call for every single file in the directory, resulting in a massive number of network round-trips (the "N+1 query problem").

The system uses this module to solve that performance bottleneck. By defining the `ReadDirPlus` trait, the VFS can return a "fat" directory listing where each `Entry` includes the `file::Attr` and `file::Handle` inline. Inside the system, the RPC handler receives a `READDIRPLUS` request and invokes the `read_dir_plus` method. The implementation is responsible for reading the directory stream (using `Cookie` and `CookieVerifier` from the `read_dir` module) and fetching the necessary metadata for each entry. It must carefully manage the response size using the dual constraints of `dir_count` (for the name data) and `max_count` (for the total payload) to ensure the response fits within network packets while maximizing the amount of useful data returned. Without this module, the NFS server would be significantly less efficient, as clients would have to churn the network with attribute lookups for every file in a directory listing.