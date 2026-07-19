<!-- SPEC_HASH: 15c3ab6b3a0cc115a3b906d59a6be8783db55b2c8f62ac5286b4065646d5174e -->
# Module Specification

Module: nfs_mamont::vfs::read_dir
Rust File: src/vfs/read_dir.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **crate::consts::nfsv3**: Used to import the `NFS3_COOKIEVERFSIZE` constant. This constant defines the fixed size (8 bytes) of the `CookieVerifier` array, ensuring the structure matches the NFSv3 protocol specification (RFC 1813) for directory entry verification.
- **crate::vfs**: Used to import the `Error` enum. Specifically, `vfs::Error::BadCookie` is referenced in the documentation of the `ReadDir` trait to indicate the failure condition when a directory cookie is no longer valid.
- **super::file**: Used to import the `Handle`, `Name`, and `Attr` types. These types are essential for identifying the target directory (`Handle`), naming directory entries (`Name`), and returning directory metadata (`Attr`) in both success and failure cases.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define the interface and data structures required to implement the NFSv3 `READDIR` procedure.
- To provide a mechanism for stateless directory traversal using opaque cookies and verifiers, allowing clients to resume reading directories from a specific point while ensuring the directory has not been modified in the interim.
- To encapsulate the constraints of the NFSv3 protocol regarding directory entry sizes and attribute propagation.

Inputs:
- `Args`: A structure containing the directory `Handle`, a `Cookie` (offset), a `CookieVerifier` (state check), and a `count` (maximum response size in bytes).

Outputs:
- `Result<Success, Fail>`:
  - `Success`: Contains the directory attributes, a new `CookieVerifier`, a vector of `Entry` objects, and an `eof` flag.
  - `Fail`: Contains a `vfs::Error` and optional directory attributes.

Steps:
1. **Cookie Initialization**: The `Cookie` struct wraps a `u64`. The value `0` is reserved to indicate the start of the directory. The `is_zero` method is used to check for this initial state.
2. **Verifier Validation**: The `CookieVerifier` struct wraps an 8-byte array (`[u8; NFS3_COOKIEVERFSIZE]`). It acts as a signature of the directory's state. A verifier of all zeros (`is_zero`) indicates the start of a read sequence.
3. **Entry Definition**: The `Entry` struct represents a single directory listing. It contains a `file_id` (unique identifier), `file_name`, and a `cookie` which points to the *next* entry in the directory, allowing the client to continue iteration.
4. **Interface Execution**: The `ReadDir` trait defines the asynchronous `read_dir` method. Implementers are expected to:
   - Validate the `cookie_verifier` against the current state of the directory identified by `args.dir`.
   - If the verifier is invalid, return `Err(Fail)` with `vfs::Error::BadCookie`.
   - Read entries starting from the position indicated by `args.cookie`.
   - Stop reading when the accumulated size of the response (including XDR overhead) would exceed `args.count` or the end of the directory is reached.
   - Return `Ok(Success)` with the retrieved entries, a new verifier for the next request, and `eof` set to `true` if the end of the directory was reached.

Edge Cases:
- **Zero Cookie/Verifier**: Both `Cookie` and `CookieVerifier` use a zero value to signify the initial request. The logic must distinguish between a user-provided zero (start) and a server-provided zero (end of stream, though usually `eof` handles this).
- **Empty Directory**: The `read_dir` call should return `Success` with an empty `entries` vector and `eof` set to `true`.
- **Stale Cookie**: If the directory is modified between calls, the `cookie_verifier` will change, and the implementation must return `BadCookie`.

Complexity:
- Time: O(N) where N is the number of entries read, determined by the implementation of the `ReadDir` trait.
- Space: O(N) for the `Vec<Entry>` in the `Success` struct, where N is the number of entries returned in the response.

Determinism:
- Deterministic (assuming the underlying file system state remains consistent or changes are detected via the verifier).

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::consts::nfsv3`**: Provides the `NFS3_COOKIEVERFSIZE` constant (8 bytes). This is critical for defining the memory layout of the `CookieVerifier` struct, ensuring it matches the wire format defined in RFC 1813.
- **From `nfs_mamont::vfs::file`**: Provides `Handle`, `Name`, and `Attr`.
  - `Handle` is used in `Args` to specify which directory to read.
  - `Name` is used in `Entry` to return the name of the file/directory.
  - `Attr` is used in `Success` and `Fail` to return the attributes of the directory being read (Weak Cache Consistency).
- **From `nfs_mamont::vfs`**: Provides the `Error` enum. The specific variant `vfs::Error::BadCookie` is the mandated error type for `Fail` when the `cookie_verifier` does not match the directory's current state.

---

## 4. Data Model

Entities:
- **Cookie**: A wrapper around a `u64` representing an opaque offset or position within a directory.
- **CookieVerifier**: A wrapper around an array of 8 bytes (`[u8; 8]`) representing the state of the directory at the time the cookie was generated.
- **Entry**: A structure representing a file system object within a directory. It contains `file_id` (u64), `file_name` (file::Name), and `cookie` (Cookie).
- **Args**: Arguments passed to the `read_dir` function, containing the directory handle, current cookie, current verifier, and maximum byte count.
- **Success**: The successful result of a `read_dir` operation, containing directory attributes, a new verifier, a list of entries, and an EOF flag.
- **Fail**: The failure result of a `read_dir` operation, containing a `vfs::Error` and optional directory attributes.

Relations:
- **Composition**: `Args` aggregates `Cookie` and `CookieVerifier`.
- **Composition**: `Entry` aggregates `Cookie` (specifically, the cookie for the *next* entry).
- **Composition**: `Success` aggregates a vector of `Entry` and a `CookieVerifier`.
- **Association**: `Fail` is associated with `vfs::Error`.

Global Invariants:
- The `CookieVerifier` array is always exactly 8 bytes long (`NFS3_COOKIEVERFSIZE`).
- A `Cookie` value of 0 always implies the start of the directory.
- A `CookieVerifier` of all zeros always implies the start of the read sequence.
- The `file_id` in an `Entry` should not be 0 to avoid confusion with special meanings in UNIX clients (as per RFC 1813).

## 5. Error Model

Error Types:
- `vfs::Error` (specifically `vfs::Error::BadCookie` is documented for invalid verifiers).

Error Propagation Strategy:
- The `read_dir` function returns a `Result<Success, Fail>`. Errors are wrapped in the `Fail` struct, which includes the specific `vfs::Error` and optionally the directory attributes (`dir_attr`) to allow the client to update its cache even on failure.

Recoverability:
- Recoverable. If `BadCookie` is returned, the client must restart the directory read from the beginning (cookie 0). Other errors (e.g., `IO`, `Access`) imply standard file system error handling.

Panics:
- Allowed: No.
- Conditions: This module defines data structures and a trait interface. It contains no executable logic that could result in a panic.

---

## 6. Traits

List which external traits this module implements:
- **Send**: The `ReadDir` trait is transformed into a Send-safe object trait via the `#[trait_variant::make(Send)]` attribute macro, allowing it to be passed between threads.

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here you MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to define the contract for reading directory contents in the `nfs_mamont` NFSv3 server implementation. The system contains a complex Virtual File System (VFS) layer that abstracts the underlying storage (e.g., local disk, network storage) and presents it via the NFS protocol. A typical usage scenario of the system involves a client requesting a list of files in a directory. Since directories can contain thousands of files and network packets have size limits (MTU), the entire listing cannot be sent in a single response. The system uses the `read_dir` module to manage this fragmentation: the server sends a batch of entries along with a `Cookie` (an opaque marker representing the position in the directory stream) and a `CookieVerifier` (a checksum of the directory's state to ensure it hasn't changed).

Inside the system, the following things happen and they use this module: The RPC handler receives a `READDIR` request and deserializes the arguments into the `Args` struct defined here. It then invokes the `read_dir` method on the VFS implementation. The VFS implementation uses the `file::Handle` to locate the directory, checks the `CookieVerifier` to ensure the directory hasn't been modified since the last request (returning `vfs::Error::BadCookie` if it has), and then iterates through the directory entries starting at the offset indicated by the `Cookie`. It populates the `Success` struct with `Entry` objects (which include the next `Cookie` for each entry) until the byte count limit is reached. Without this module, the VFS would lack a standardized way to handle the stateful resumption of directory reads required by the NFSv3 protocol, leading to potential data corruption or protocol violations if the directory changes during a read operation.