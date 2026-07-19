<!-- SPEC_HASH: ef5bd721d695c1963fcf35d806672d01b03e9392b6d3d29696f49b192484dc5d -->
# Module Specification

Module: nfs_mamont::mount::mnt
Rust File: /home/yarovoy/Documents/yadro/projects/oss/github/nfs-mamont/nfs-mamont/src/mount/mnt.rs

---

## 1. Dependencies

From the code analysis, the following external crates and standard library modules are used:

- **`std::net::SocketAddr`**: Used to identify the network address of the client initiating the mount request. This is necessary for logging, access control, or tracking mount records per client IP.
- **`num_derive` (and implicitly `num_traits`)**: Used to derive `FromPrimitive` and `ToPrimitive` on the `Fail` enum. This allows the module to convert between the integer error codes defined in the MOUNT protocol (RFC 1813) and the Rust enum variants.
- **`crate::rpc`**: Used to import `AuthFlavor` and `OpaqueAuth`. `AuthFlavor` is used to indicate which authentication mechanisms the server supports for the mounted directory. `OpaqueAuth` is used to pass the client's credentials to the mount procedure.
- **`crate::vfs::file`**: Used to import `Handle` and `Path`. `Handle` represents the opaque file handle returned to the client upon a successful mount. `Path` is used to represent the directory path requested by the client.

*Assumption*: The `vfs::file` module provides `Handle` as a fixed-size byte array (`[u8; NFS3_FHSIZE]`) and `Path` as a validated wrapper around a filesystem path string, as inferred from the dependency facts.

---

## 2. Mechanics

This module defines the interface for the MOUNT version 3 protocol, specifically Procedure 1 (MNT). It provides the data structures and the trait definition required to implement the server-side logic for mapping a directory path to an NFS file handle.

Intent:
- To strictly model the data types and procedure signatures defined in RFC 1813 section 5.2.1.
- To abstract the implementation details of the mounting process behind the `Mnt` trait, allowing the RPC layer to interact with the filesystem logic without knowing the specifics of path resolution or handle generation.

Inputs:
- `Args`: A structure containing the directory path (`file::Path`) the client wishes to mount.
- `SocketAddr`: The network address of the client.
- `OpaqueAuth`: The authentication credentials provided by the client.

Outputs:
- `Result<Success, Fail>`: The result of the mount operation. `Success` contains the file handle and supported authentication flavors. `Fail` contains a protocol-specific error code.

Steps:
1.  **Decode Request**: The RPC layer decodes the incoming bytes into `Args`, `SocketAddr`, and `OpaqueAuth`.
2.  **Execute Mount**: The `mnt` method of the `Mnt` trait is invoked.
3.  **Path Resolution**: The implementation resolves the provided `dirpath` to a filesystem entity.
4.  **Validation**: The implementation checks if the entity is a directory and if the client has permission to access it.
5.  **Handle Generation**: If valid, a `file::Handle` is generated or retrieved.
6.  **State Update**: The server records the mount (e.g., adds an entry to the mount list).
7.  **Response**: The method returns `Success` with the handle and supported `AuthFlavor`s, or `Fail` with an appropriate error code.

Edge Cases:
- The requested path does not exist (`Fail::NoEnt`).
- The requested path is not a directory (`Fail::NoDir`).
- The client lacks permission (`Fail::Access` or `Fail::Perm`).
- The path name is too long (`Fail::NameTooLong`).

Complexity:
- Time: N/A (Interface definition).
- Space: N/A (Interface definition).

Determinism:
- Deterministic (Interface definition).

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::rpc`**:
    - **`AuthFlavor`**: An enum identifying authentication types (e.g., `None`, `Sys`, `RpcSecGss`). This is used in the `Success` struct to inform the client which authentication methods are valid for the returned file handle.
    - **`OpaqueAuth`**: A struct containing an authentication flavor and a byte vector body. This is passed to the `mnt` function to represent the client's credentials.
- **From `nfs_mamont::vfs::file`**:
    - **`Handle`**: A struct representing a file handle (opaque identifier). This is the primary output of the mount operation, used by the client in subsequent NFS protocol operations.
    - **`Path`**: A struct representing a server pathname. This is the primary input for the mount operation.

---

## 4. Data Model

Entities:
- **`Fail`**: An enum representing MOUNT protocol error codes. Each variant maps to a specific integer value defined in RFC 1813 (e.g., `Perm = 1`, `NoEnt = 2`).
- **`Success`**: A struct representing a successful mount response.
    - `file_handle`: The `file::Handle` for the mounted directory.
    - `auth_flavors`: A vector of `AuthFlavor` indicating supported authentication methods.
- **`Args`**: A struct representing the mount request arguments.
    - `dirpath`: The `file::Path` of the directory to mount.
- **`Mnt`**: A trait defining the asynchronous mount procedure.

Relations:
- `Success` → `file::Handle` (Composition)
- `Success` → `Vec<AuthFlavor>` (Composition)
- `Args` → `file::Path` (Composition)
- `Mnt::mnt` returns `Result<Success, Fail>`

Global Invariants:
- The discriminants of the `Fail` enum must match the values specified in RFC 1813.
- The `Mnt` trait is `Send`, meaning the implementor can be safely shared across threads.

---

## 5. Error Model

Error Types:
- **`Fail`**: An enum covering specific MOUNT protocol errors:
    - `Perm`: Not owner.
    - `NoEnt`: No such file or directory.
    - `Io`: I/O error.
    - `Access`: Permission denied.
    - `NoDir`: Not a directory.
    - `Inval`: Invalid argument.
    - `NameTooLong`: Filename too long.
    - `NotSupp`: Operation not supported.
    - `ServerFault`: A failure on the server.

Error Propagation Strategy:
- Custom enum (`Fail`). The module defines its own error type specific to the MOUNT protocol, distinct from the VFS or RPC error layers, though the values are semantically similar to standard POSIX errors.

Recoverability:
- Context-dependent. Errors like `NoEnt` or `Access` are typically fatal for the specific request but allow the client to retry with a different path or credentials. `ServerFault` indicates a server-side issue that might require administrative intervention.

Panics:
- Allowed: No (This module only defines types and a trait interface).

---

## 6. Traits

The module defines and implements the following traits:

- **`Mnt`**: The primary trait defining the `mnt` asynchronous procedure. It is marked `Send` via `#[trait_variant::make(Send)]`.
- **`Debug`**: Derived on `Fail` and `Args`.
- **`ToPrimitive`**: Derived on `Fail`.
- **`FromPrimitive`**: Derived on `Fail`.
- **`Eq`, `PartialEq`**: Derived on `Args` (in `test` cfg).

---

## 7. Overview

This module is used in order to define the contract for the MOUNT protocol (version 3) within the `nfs_mamont` NFS server implementation. The MOUNT protocol is a prerequisite for the NFS protocol; while NFS operates on opaque file handles to access files, clients initially only possess human-readable directory paths. This module bridges that gap by defining the interface that converts a path into a file handle.

This system contains the data structures (`Args`, `Success`, `Fail`) that strictly adhere to the RFC 1813 specification for the MOUNT protocol. It separates the definition of the protocol messages from the logic of handling them, allowing the RPC layer to serialize/deserialize these types while the VFS layer provides the implementation of the `Mnt` trait.

A typical usage scenario of the system involves a client sending a MOUNT request for a directory (e.g., "/export/data"). The RPC layer decodes this request into an `Args` struct and an `OpaqueAuth` credential. It then invokes the `mnt` method on an object implementing the `Mnt` trait. The implementation verifies the path exists, is a directory, and the user is authorized. If successful, it returns a `Success` struct containing a `file::Handle` (a byte array) and a list of supported `AuthFlavor`s. The client then uses this file handle for subsequent NFS operations (READ, WRITE, etc.).

Inside the system, the following things happen and they use:
- **Protocol Translation**: Uses `Fail` and `Success` structs to map between Rust logic and the binary MOUNT protocol format.
- **Authentication Negotiation**: Uses `AuthFlavor` from the `rpc` module to agree on security mechanisms.
- **Path Resolution**: Uses `file::Path` from the `vfs` module to accept the target directory and `file::Handle` to return the server's internal identifier for that directory.