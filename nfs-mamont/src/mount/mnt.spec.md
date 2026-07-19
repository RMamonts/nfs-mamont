<!-- SPEC_HASH: ef5bd721d695c1963fcf35d806672d01b03e9392b6d3d29696f49b192484dc5d -->
# Module Specification

Module: nfs_mamont::mount::mnt
Rust File: src/mount/mnt.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`std::net::SocketAddr`**: Used to pass the client's network address to the `mnt` procedure. This allows the implementation to perform access control checks based on the IP address of the requester.
- **`num_derive`**: Used to derive `FromPrimitive` and `ToPrimitive` for the `Fail` enum. This is essential for mapping the integer error codes defined in the MOUNT protocol (RFC 1813) to Rust enum variants during serialization and deserialization.
- **`crate::rpc::{AuthFlavor, OpaqueAuth}`**: Used to define the authentication context. `AuthFlavor` enumerates the supported authentication mechanisms (e.g., `Sys`, `None`), and `OpaqueAuth` wraps the raw credential bytes sent by the client.
- **`crate::vfs::file`**: Used to import `file::Handle` and `file::Path`. `file::Handle` is the opaque identifier returned to the client upon a successful mount, and `file::Path` is the validated type used to represent the directory path requested by the client.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define the Rust interface for the MOUNT protocol version 3, Procedure 1 (MNT), as specified in RFC 1813.
- To provide a strongly-typed contract that maps the raw protocol data structures (paths, file handles, authentication flavors) to Rust types, ensuring that implementations adhere to the expected behavior of resolving a directory path to a file handle.

Inputs:
- **`args: Args`**: Contains the `dirpath` (`file::Path`) representing the server-side directory the client wishes to mount.
- **`client_addr: SocketAddr`**: The network address of the client.
- **`cred: OpaqueAuth`**: The authentication credentials provided by the client.

Outputs:
- **`Result<Success, Fail>`**: Indicates the outcome of the mount operation.
    - `Ok(Success)`: Contains the `file_handle` for the directory and a list of supported `auth_flavors`.
    - `Err(Fail)`: Contains a specific MOUNT protocol error code.

Steps:
1. **Error Mapping**: The `Fail` enum is defined with explicit discriminants (e.g., `Perm = 1`, `NoEnt = 2`) corresponding to the status values in RFC 1813. The `num_derive` macros facilitate conversion between these integers and the enum variants.
2. **Argument Structure**: The `Args` struct wraps a `file::Path`. By using `file::Path`, the module ensures that the directory path has already been validated (e.g., length limits) before reaching the implementation logic.
3. **Success Structure**: The `Success` struct aggregates two critical pieces of information:
    - `file_handle`: The `file::Handle` that the client must use in subsequent NFS protocol calls.
    - `auth_flavors`: A `Vec<AuthFlavor>` indicating which authentication mechanisms the server accepts for this specific mount.
4. **Trait Definition**: The `Mnt` trait defines the asynchronous `mnt` method. The `#[trait_variant::make(Send)]` attribute ensures that the trait object is safe to send across threads, which is necessary for asynchronous server environments. The method signature explicitly requires the client address and credentials, enabling the implementer to enforce security policies.

Edge Cases:
- **`Fail::ServerFault` (10006)**: Defined as a catch-all for unspecified server-side errors, distinct from standard I/O errors.
- **`Fail::NotSupp` (10004)**: Indicates that the operation is not supported, allowing the server to decline a mount request for protocol-specific reasons.

Complexity:
- **Time**: O(1) for type definitions and trait dispatch. The complexity of the actual mount operation depends on the implementation of the `Mnt` trait.
- **Space**: O(N) for `Success::auth_flavors`, where N is the number of supported authentication flavors. O(1) for other structures.

Determinism:
- **Deterministic**: The module defines data structures and interfaces; it contains no runtime logic that introduces non-determinism.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::file`**:
    - **`file::Path`**: This module relies on the validation guarantees provided by `file::Path`. Specifically, it assumes that the `dirpath` passed in `Args` is a valid, non-empty string that does not exceed the maximum path length defined by the VFS layer.
    - **`file::Handle`**: This module uses `file::Handle` as the return value for a successful mount. It treats the handle as an opaque byte array (`[u8; NFS3_FHSIZE]`) that uniquely identifies the mounted directory to the client.

- **From `nfs_mamont::rpc`**:
    - **`AuthFlavor`**: This module uses the `AuthFlavor` enum to list the authentication methods supported by the server. The `Success` struct returns a vector of these flavors, allowing the client to choose a compatible security mechanism.
    - **`OpaqueAuth`**: This module uses `OpaqueAuth` to transport the client's credentials. The `mnt` method accepts this struct, allowing the implementation to inspect the `flavor` and `body` to verify the client's identity.

---

## 4. Data Model

Entities:
- **`Fail`**: An enumeration of MOUNT protocol error codes.
    - Variants include `Perm`, `NoEnt`, `Io`, `Access`, `NoDir`, `Inval`, `NameTooLong`, `NotSupp`, `ServerFault`.
- **`Success`**: A structure representing a successful MNT response.
    - `file_handle`: `file::Handle` (The file handle for the mounted directory).
    - `auth_flavors`: `Vec<AuthFlavor>` (List of supported authentication flavors).
- **`Args`**: A structure representing the MNT request arguments.
    - `dirpath`: `file::Path` (The server pathname of the directory).
- **`Mnt`**: A trait defining the interface for the MNT procedure.
    - `mnt`: Async function taking `Args`, `SocketAddr`, and `OpaqueAuth`, returning `Result<Success, Fail>`.

Relations:
- **`Mnt` trait uses `Args`**: The `mnt` method consumes `Args` to get the target path.
- **`Mnt` trait returns `Success`**: A successful operation produces a `Success` struct.
- **`Success` contains `file::Handle`**: The result includes the file handle derived from the VFS.
- **`Success` contains `Vec<AuthFlavor>`**: The result includes authentication metadata from the RPC layer.

Global Invariants:
- **`Fail` Discriminants**: The integer values of `Fail` variants are fixed and must match RFC 1813 (e.g., `NoEnt` is always 2).
- **`Success` Validity**: If `mnt` returns `Ok(Success)`, the `file_handle` must be valid for use in the NFS protocol, and `auth_flavors` should contain at least one entry if the server expects authentication.

## 5. Error Model

Error Types:
- **`Fail`**: A public enum representing the specific error conditions defined by the MOUNT protocol.

Error Propagation Strategy:
- **Direct Return**: The `mnt` method in the `Mnt` trait returns `Result<Success, Fail>`. Errors are explicitly typed via the `Fail` enum rather than using a generic error type like `anyhow::Error` or `std::io::Error`.

Recoverability:
- **Recoverable**: The client receives the specific `Fail` variant and can decide whether to retry (e.g., in case of `Io` or `ServerFault`) or abort the operation (e.g., in case of `Perm` or `Access`).

Panics:
- **Allowed**: No. The interface definition does not perform operations that can panic. Implementations of the trait are expected to handle errors by returning `Err(Fail)`.

---

## 6. Traits

List which external traits this module implements:
- **`std::fmt::Debug`**: Implemented for `Fail` and `Args`.
- **`num_traits::ToPrimitive`**: Implemented for `Fail`.
- **`num_traits::FromPrimitive`**: Implemented for `Fail`.
- **`std::marker::Send`**: Implemented for `Mnt` (via `#[trait_variant::make(Send)]`).

List which traits this module defines:
- **`Mnt`**: The core trait defining the MOUNT protocol procedure.

---

## 7. Overview

This module is used in order to **define the interface for the MOUNT protocol, which serves as the entry point for clients to access the NFS server's file system**. The system contains a complex implementation of an NFSv3 server where the MOUNT protocol is distinct from the NFS protocol itself. While the NFS protocol (defined in the VFS layer) operates on opaque file handles, the MOUNT protocol is responsible for resolving human-readable directory paths (e.g., "/export/home") into these opaque handles.

A typical usage scenario of the system involves a client connecting to the server and requesting to mount a specific directory. The server receives this request, which is dispatched to an implementation of the `Mnt` trait defined in this module. The implementation uses the `Args` (containing the `file::Path`) to look up the directory in the underlying file system. It checks the `client_addr` and `cred` (from `OpaqueAuth`) to ensure the client is authorized to access the path. If successful, it returns a `Success` struct containing a `file::Handle` and a list of `AuthFlavor`s. The client then uses this `file::Handle` in all subsequent NFS protocol requests (READ, WRITE, LOOKUP, etc.) to refer to that directory.

Inside the system, the following things happen and they use this module:
1.  **Protocol Translation**: The RPC layer receives a raw MOUNT request packet. It deserializes the arguments into the `Args` struct defined here. The `file::Path` inside `Args` ensures the path string is validated against VFS limits before the logic even runs.
2.  **Security Negotiation**: The `Success` struct defined here is critical for negotiating the security parameters. By returning a `Vec<AuthFlavor>`, the server tells the client which authentication mechanisms (e.g., `AUTH_SYS`, `RPCSEC_GSS`) are acceptable for the session.
3.  **Service Abstraction**: The `Mnt` trait allows the server to decouple the protocol handling (RPC) from the business logic (resolving paths and checking permissions). The main server loop can hold a generic `Arc<dyn Mnt + Send + Sync>` and invoke `mnt` without knowing the specifics of the export configuration or backend storage.

Without this module, the system would lack a standardized, type-safe way to handle the initial client handshake. The mapping between string paths and file handles would be ad-hoc, potentially leading to inconsistencies between the MOUNT service and the NFS service, and security checks based on client address or credentials would not be formally enforced in the contract.