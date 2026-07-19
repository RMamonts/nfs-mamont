<!-- SPEC_HASH: 4b9836872dd3a818c0246d98f818cc5490753c7bbd36f67efb371a796a9c1011 -->
# Module Specification

Module: nfs_mamont::vfs::path_conf
Rust File: src/vfs/path_conf.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`crate::vfs`**: Used to import the `vfs::Error` enum. This allows the `Fail` struct to report errors using the standardized error codes defined for the VFS layer, which map directly to NFSv3 status codes.
- **`super::file`**: Used to import `file::Handle` and `file::Attr`. `Handle` is required in `Args` to identify the target file system object. `Attr` is used in `Success` and `Fail` to return the metadata of the object, adhering to the NFSv3 pattern of returning attributes alongside operation results.
- **`trait_variant`**: Used via the `#[trait_variant::make(Send)]` attribute on the `PathConf` trait. This macro generates an `async` trait that is also object-safe and implements `Send`, allowing the trait to be used as a trait object in multi-threaded, asynchronous contexts (e.g., within a `Box<dyn PathConf>`).

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define the interface for the NFSv3 `PATHCONF` procedure (RFC 1813, Section 3.3.7).
- To allow clients to retrieve configurable limits and behavioral characteristics of the file system associated with a specific file or directory.
- To provide metadata (attributes) alongside the configuration data, enabling clients to update their caches efficiently.

Inputs:
- **`Args`**: A structure containing a `file::Handle`, which uniquely identifies the file system object (file or directory) being queried.

Outputs:
- **`Result<Success, Fail>`**:
    - **`Success`**: Contains the pathconf information (limits and flags) and optionally the current attributes of the object.
    - **`Fail`**: Contains a `vfs::Error` describing the failure and optionally the attributes of the object (if the handle was valid but the operation failed).

Steps:
1. **Invocation**: The RPC layer or a consumer calls the `path_conf` method with a `Args` struct containing the target file handle.
2. **Resolution**: The implementation of the trait resolves the `file::Handle` to a specific file system object.
3. **Retrieval**: The implementation queries the underlying file system for the specific limits (e.g., maximum filename length, maximum link count) and behavior flags (e.g., case sensitivity, chown restrictions).
4. **Attribute Fetching**: The implementation attempts to fetch the current `file::Attr` for the object. This is included in the result if available.
5. **Response Construction**:
    - If successful, a `Success` struct is populated with the retrieved data and attributes.
    - If an error occurs (e.g., stale handle, I/O error), a `Fail` struct is populated with the error and any available attributes.
6. **Return**: The `Result` is returned to the caller.

Edge Cases:
- **Optional Attributes**: The `file_attr` field in both `Success` and `Fail` is `Option<file::Attr>`. This handles cases where the file system object exists but its attributes cannot be retrieved, or where the protocol semantics allow omitting attributes.
- **`no_trunc` Behavior**: The `no_trunc` flag in `Success` explicitly defines how the server handles filenames exceeding `name_max`. If `true`, the server rejects them (likely with `NameTooLong`); if `false`, it silently truncates them. This is a critical behavioral distinction for clients.

Complexity:
- **Time**: The complexity is determined by the implementation of the trait, specifically the cost of resolving the handle and querying file system metadata. The interface definition itself is O(1).
- **Space**: O(1) for the structures defined in this module.

Determinism:
- **Deterministic**: The interface defines a pure function signature. The determinism of the output depends entirely on the implementation of the trait and the state of the underlying file system.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::file`**:
    - **`Handle`**: Used as the input key in `Args`. It serves as the opaque identifier that the VFS backend uses to locate the file or directory.
    - **`Attr`**: Used in the return types (`Success` and `Fail`). This allows the `PATHCONF` operation to double as a lightweight attribute check, returning the object's metadata (size, mode, times) along with the configuration limits, which is standard practice in NFSv3 to minimize round trips.

- **From `nfs_mamont::vfs`**:
    - **`Error`**: Used in the `Fail` struct. This ensures that errors reported by the `PathConf` operation are consistent with the rest of the VFS layer (e.g., `StaleFile`, `IO`, `Access`), allowing the RPC layer to map them correctly to NFS status codes.

---

## 4. Data Model

Entities:
- **`Args`**: Arguments for the pathconf operation.
    - `file`: `file::Handle` (The target object).
- **`Success`**: Successful result of the operation.
    - `file_attr`: `Option<file::Attr>` (Post-operation attributes).
    - `link_max`: `u32` (Maximum hard links).
    - `name_max`: `u32` (Maximum filename length).
    - `no_trunc`: `bool` (Reject vs. truncate long names).
    - `chown_restricted`: `bool` (Restrict `chown` to privileged users).
    - `case_insensitive`: `bool` (Case-insensitive filename lookup).
    - `case_preserving`: `bool` (Preserve case when creating names).
- **`Fail`**: Failed result of the operation.
    - `error`: `vfs::Error` (The specific error encountered).
    - `file_attr`: `Option<file::Attr>` (Attributes if available).

Relations:
- **`Args` → `file::Handle`**: Composition.
- **`Success` → `file::Attr`**: Optional Composition.
- **`Fail` → `vfs::Error`**: Composition.
- **`Fail` → `file::Attr`**: Optional Composition.

Global Invariants:
- **NFSv3 Compliance**: The fields in `Success` map directly to the `pathconf3` structure in RFC 1813. The values returned must accurately reflect the properties of the file system containing the object referenced by the handle.

## 5. Error Model

Error Types:
- **`vfs::Error`**: The error type wrapped in the `Fail` struct. This enum covers standard NFS errors (e.g., `StaleFile`, `IO`, `Access`).

Error Propagation Strategy:
- **Result Wrapper**: The trait method returns `Result<Success, Fail>`. Errors are not thrown but returned explicitly in the `Err` variant.
- **Attribute Preservation**: The `Fail` struct includes `file_attr`. This allows the implementation to return attributes even if the specific pathconf query failed (e.g., if the handle is valid but the user lacks permission to read configuration), adhering to the "weak cache consistency" model where attributes are returned whenever possible.

Recoverability:
- **Dependent on Error**: Recoverability depends on the specific `vfs::Error` variant. For example, `JUKEBOX` implies the client should retry, while `StaleFile` requires the client to perform a new lookup.

Panics:
- **Allowed**: No. The interface is designed for async network operations; panics would crash the server task or thread.

---

## 6. Traits

List which external traits this module implements:
- **`Send`**: The `PathConf` trait is transformed by `trait_variant` to implement `Send`, allowing it to be used as a trait object across thread boundaries.

List which traits this module defines:
- **`PathConf`**: The core trait defining the `path_conf` asynchronous method.

---

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than this level.

This module is used in order to **expose file system configuration limits and behavioral characteristics to NFS clients**. In the context of the `nfs_mamont` NFSv3 server, the system must handle a wide variety of client expectations and underlying file system behaviors (e.g., case-insensitive FAT vs. case-sensitive ext4). The `PathConf` interface is the mechanism by which the server informs the client about these specific rules, ensuring the client can adapt its operations (like file creation or renaming) to the server's constraints without causing errors.

The system contains a complex architecture where the VFS layer abstracts the storage backend. The `PathConf` trait is a specific procedure within the broader `Vfs` facade. It allows the RPC layer to query a file handle and receive structured data about limits (e.g., `name_max`) and semantics (e.g., `chown_restricted`).

A typical usage scenario of the system involves a client connecting to the server and intending to upload a file with a very long name. Before attempting the upload, the client invokes the `PATHCONF` procedure on the target directory. The server receives the request, extracts the file handle from `Args`, and calls the `path_conf` method. The implementation checks the underlying file system, determines that `name_max` is 255 and `no_trunc` is true, and returns `Success` with these values. The client sees this and truncates the name locally to 255 characters before sending the `CREATE` request, avoiding a `NameTooLong` error.

Inside the system, the following things happen and they use this module:
1.  **Protocol Compliance**: The `PathConf` trait ensures that the server can satisfy the requirements of RFC 1813 Section 3.3.7. The fields in `Success` (link_max, name_max, etc.) map 1:1 to the specification, ensuring that the server's response is valid and interpretable by any standard NFSv3 client.
2.  **Behavioral Adaptation**: The flags `case_insensitive` and `case_preserving` are critical for clients to correctly display and manage files. For instance, if `case_insensitive` is true, the client knows that "File.txt" and "file.txt" refer to the same object and should update its UI accordingly.
3.  **Security Enforcement**: The `chown_restricted` flag informs the client whether it can change file ownership. If true, the client knows that only the root user (UID 0) can perform `chown`, preventing the client from attempting unauthorized operations.

Without this module, the server would lack a standardized way to communicate these parameters. Clients would be forced to assume generic defaults (often incorrect), leading to operation failures, data inconsistency, or security issues. This module provides the necessary "handshake" for the client to understand the server's environment.