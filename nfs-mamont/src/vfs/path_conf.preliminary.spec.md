<!-- SPEC_HASH: 4b9836872dd3a818c0246d98f818cc5490753c7bbd36f67efb371a796a9c1011 -->
# Module Specification

Module: nfs_mamont::vfs::path_conf
Rust File: src/vfs/path_conf.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **crate::vfs**: Used to import the `Error` enum. This enum defines the specific error conditions (e.g., `NameTooLong`, `Stale`, `IO`) that can occur during the `path_conf` operation and are returned within the `Fail` struct.
- **super::file**: Used to import `Handle` and `Attr`. `Handle` is required in `Args` to identify the target file system object. `Attr` is used in both `Success` and `Fail` structs to return the current attributes of the object, allowing the client to update its cache without making a separate `GETATTR` call.
- **trait_variant**: Used via the `#[trait_variant::make(Send)]` attribute to automatically generate a `Send` version of the `PathConf` trait. This is necessary to allow the trait to be used as a trait object in asynchronous contexts that require thread safety (e.g., `dyn PathConf + Send`).

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define the interface for retrieving file system specific limits and optional characteristics (pathconf information) for a given file handle, as required by the NFSv3 protocol (RFC 1813, Section 3.3.18).
- To allow the VFS implementation to communicate static or dynamic properties of the file system (such as maximum filename length or case sensitivity) to the client.
- To provide a mechanism to return file attributes alongside the pathconf data or error, optimizing network traffic by avoiding a subsequent `GETATTR` request.

Inputs:
- `Args`: A structure containing a `file::Handle` representing the file system object (file or directory) being queried.

Outputs:
- `Result<Success, Fail>`:
  - `Success`: Contains the requested pathconf variables (`link_max`, `name_max`, `no_trunc`, `chown_restricted`, `case_insensitive`, `case_preserving`) and optionally the current `file::Attr`.
  - `Fail`: Contains a `vfs::Error` describing the failure and optionally the `file::Attr` if attributes were available before the failure occurred.

Steps:
1. The `PathConf::path_conf` method is invoked with a reference to `Args`.
2. The implementation attempts to resolve the provided `file::Handle` to a valid file system object.
3. The implementation retrieves the file system properties associated with the object's underlying file system.
4. The implementation attempts to retrieve the current attributes (`file::Attr`) of the object.
5. If the operation is successful, the implementation returns `Ok(Success)` populated with the properties and the attributes (if available).
6. If the operation fails (e.g., invalid handle, I/O error), the implementation returns `Err(Fail)` populated with the specific `vfs::Error` and any attributes that were successfully retrieved (if applicable).

Edge Cases:
- **Attribute Availability**: The `file_attr` field in both `Success` and `Fail` is `Option<file::Attr>`. This implies that if the object is deleted or the handle becomes stale immediately after lookup, attributes might not be available even if the pathconf properties are retrieved, or vice versa.
- **Cross-Filesystem Properties**: Since the `Args` contains a handle to a specific file, the properties returned should theoretically correspond to the file system containing that specific file. If the server supports multiple file system types with different limits, the implementation must return the limits specific to the target file's file system.

Complexity:
- Time: Dependent on the implementation of the `PathConf` trait. Typically involves a lookup by handle (O(1) or O(log N)) and attribute retrieval.
- Space: O(1) for the returned structures.

Determinism:
- Deterministic (Interface definition).

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `crate::vfs::file`**:
  - **Handle**: Used as the input key in `Args` to uniquely identify the file system object. The `Handle` is a fixed-size opaque byte array (`[u8; NFS3_FHSIZE]`).
  - **Attr**: Used as the payload for metadata in the `Success` and `Fail` structs. It contains comprehensive file metadata (type, mode, uid, gid, size, timestamps, etc.).
- **From `crate::vfs`**:
  - **Error**: Used to classify the failure reason in the `Fail` struct. This enum maps specific error conditions (like `NameTooLong` or `StaleFile`) to integer codes used in the NFS protocol.

---

## 4. Data Model

Entities:
- **Success**: Represents a successful `PATHCONF3` response.
  - `file_attr`: Optional attributes of the object.
  - `link_max`: Maximum number of hard links to a file.
  - `name_max`: Maximum length of a filename component.
  - `no_trunc`: Boolean indicating if long names are rejected or truncated.
  - `chown_restricted`: Boolean indicating if `chown` is restricted to privileged users.
  - `case_insensitive`: Boolean indicating if the filesystem is case-insensitive.
  - `case_preserving`: Boolean indicating if the filesystem preserves case in names.
- **Fail**: Represents a failed `PATHCONF3` response.
  - `error`: The specific `vfs::Error` that occurred.
  - `file_attr`: Optional attributes of the object (may be present if the error occurred after attribute retrieval).
- **Args**: Represents the arguments for the `path_conf` operation.
  - `file`: The `file::Handle` of the object to query.
- **PathConf**: A trait defining the asynchronous interface for retrieving pathconf information.

Relations:
- **Composition**: `Args` contains `file::Handle`.
- **Composition**: `Success` and `Fail` contain `Option<file::Attr>`.
- **Composition**: `Fail` contains `vfs::Error`.

Global Invariants:
- **Name Truncation**: If `no_trunc` is `true`, the server guarantees that any request creating a name longer than `name_max` will fail with `vfs::Error::NameTooLong`. If `false`, the name will be truncated.
- **Chown Restriction**: If `chown_restricted` is `true`, only the privileged user (UID 0) can change the owner or group of a file.

## 5. Error Model

Error Types:
- `vfs::Error` (imported from `crate::vfs`).

Error Propagation Strategy:
- The trait method returns `Result<Success, Fail>`. The `Fail` struct wraps the `vfs::Error`. This allows the caller to distinguish between different types of failures (e.g., `StaleFile` vs `IO`) while also potentially receiving cached attributes.

Recoverability:
- Recoverable. The client receives the error code and can decide whether to retry, abort, or use the provided attributes (if any) to update its cache.

Panics:
- Allowed: No (Interface definition).

---

## 6. Traits

List which external traits this module implements:
- **PathConf** (defined in this module): The main trait for the operation. It is marked `Send` via the `trait_variant` macro.

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here you MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to define the interface for the NFSv3 `PATHCONF` procedure within the Virtual File System (VFS) layer. The system contains a complex implementation of an NFSv3 server that must translate between raw network packets and high-level file system operations. A typical usage scenario of the system involves a client connecting to the server and needing to determine the constraints of the exported file system, such as the maximum filename length or whether the file system is case-sensitive, before performing operations like file creation or renaming. The system uses the `PathConf` trait defined in this module to abstract this query. The trait accepts a `file::Handle` (identifying the specific file or directory) and returns a `Success` struct containing the limits and flags, or a `Fail` struct containing an error.

Inside the system, the following things happen and they use this module: The NFS server receives a `PATHCONF` request from the network. It deserializes the request into the `Args` struct (containing the `file::Handle`). The server then invokes the `path_conf` method on the VFS implementation (which implements the `PathConf` trait). The VFS implementation looks up the file system object corresponding to the handle, retrieves the specific properties of the underlying file system (e.g., checking if it is an NTFS volume with case insensitivity or an EXT4 volume with case sensitivity), and fetches the current attributes. It returns this data wrapped in `Success`. If the handle is invalid (e.g., `StaleFile`), it returns `Fail`. The server then serializes this result back into an NFSv3 response packet. Without this module, the VFS would lack a standardized way to report these file system characteristics to the client, potentially leading to client-side errors when attempting operations that violate server-side constraints (like creating a filename that is too long).