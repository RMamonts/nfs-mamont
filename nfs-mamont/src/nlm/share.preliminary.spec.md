<!-- SPEC_HASH: c95dcf23a5ab342ee465fd22d76d9ce015d0d48cb6e95cf57b80c2f310f34429 -->
# Module Specification

Module: nfs_mamont::nlm::share
Rust File: src/nlm/share.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **std::io::Error**: Used to construct and return error objects when validation of the `Nlm4Share` fields fails (e.g., invalid string length) in the `new` constructor.
- **crate::consts::nlm**: Used to access the `LM_MAXSTRLEN` constant, which defines the maximum allowed length for the `caller_name` field in NLM protocol messages.
- **crate::vfs::file**: Used to import the `Handle` type, which serves as the unique identifier for the file on which the share is being requested.
- **super (nfs_mamont::nlm)**: Used to import the `OpaqueHandle` type, which represents the host or process identifier making the request.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define the data structures required to represent NLM v4 "Share" requests, which implement DOS-style file sharing semantics.
- To enforce protocol-level validation on the client identifier (`caller_name`) during the instantiation of a share request, ensuring it adheres to the length constraints defined by the NLM specification.

Inputs:
- `caller_name`: A `String` representing the hostname of the client requesting the share.
- `file_handle`: A `vfs::file::Handle` identifying the file to be shared.
- `opaque_handle`: An `OpaqueHandle` identifying the specific owner (host/process) of the request.
- `fsh4_mode`: A `FileSharingMode` enum variant indicating what operations are denied to other clients.
- `fsh4_access`: A `FileSharingAccess` enum variant indicating what operations are allowed to the requesting client.

Outputs:
- An initialized `Nlm4Share` instance.
- A `std::io::Error` of kind `InvalidInput` if validation fails.

Steps:
1. The `Nlm4Share::new` function is called with the share parameters.
2. The function checks if `caller_name` is empty. If it is, it returns an `Error` indicating the name must not be empty.
3. The function checks if the length of `caller_name` exceeds `nlm::LM_MAXSTRLEN`. If it does, it returns an `Error` indicating the name is too long.
4. If all validations pass, the function constructs and returns an `Ok` variant containing the `Nlm4Share` struct with the provided fields.

Edge Cases:
- A `caller_name` with a length exactly equal to `LM_MAXSTRLEN` is accepted.
- A `caller_name` with a length of `LM_MAXSTRLEN + 1` is rejected.

Complexity:
- Time: O(N) where N is the length of `caller_name` (due to the length check).
- Space: O(N) for storing the `caller_name` string within the struct.

Determinism:
- Deterministic

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `crate::consts::nlm`**: The constant `LM_MAXSTRLEN` is critical as it provides the specific boundary value used to validate the `caller_name` field, ensuring compliance with the NLM protocol's buffer size limits.
- **From `crate::vfs::file`**: The `Handle` type is used as a strongly-typed identifier for the file. This module relies on the `Handle` being a valid, previously constructed file identifier (though it does not validate the handle itself, only stores it).
- **From `super` (assumed `nfs_mamont::nlm`)**: The `OpaqueHandle` type is used to encapsulate the arbitrary byte sequence identifying the lock owner. This module assumes `OpaqueHandle` can be constructed and passed, storing it directly.

---

## 4. Data Model

Entities:
- **FileSharingMode**: An enumeration defining the restrictions placed on other clients. Variants are `None` (no restrictions), `Read` (deny read), `Write` (deny write), and `ReadWrite` (deny both).
- **FileSharingAccess**: An enumeration defining the permissions granted to the requesting client. Variants are `None` (no access), `Read`, `Write`, and `ReadWrite`.
- **Nlm4Share**: A structure aggregating the parameters of a share request, including the client name, file handle, owner handle, access mode, and sharing mode.

Relations:
- **Composition**: `Nlm4Share` contains instances of `FileSharingMode` and `FileSharingAccess`.
- **Association**: `Nlm4Share` holds a `vfs::file::Handle` (identifying the target file) and an `OpaqueHandle` (identifying the request owner).

Global Invariants:
- For any instance of `Nlm4Share`, the `caller_name` field is guaranteed to be non-empty and its length is guaranteed to be less than or equal to `nlm::LM_MAXSTRLEN`.
- The integer representations of `FileSharingMode` and `FileSharingAccess` variants correspond to the values 0 through 3, matching the protocol definition.

## 5. Error Model

Error Types:
- `std::io::Error`

Error Propagation Strategy:
- The module uses `std::io::Result<Self>` for the `Nlm4Share::new` constructor. Errors are constructed using `std::io::Error::new(std::io::ErrorKind::InvalidInput, message)`.

Recoverability:
- Recoverable. Callers of `Nlm4Share::new` must handle the `Result` to determine if the request parameters are valid before proceeding with lock management logic.

Panics:
- Allowed: No
- Conditions: The public API performs validation and returns errors rather than panicking on invalid input (e.g., empty strings or length overflows).

---

## 6. Traits

List which external traits this module implements:
- **std::fmt::Debug**: Implemented for `FileSharingMode`, `FileSharingAccess`, and `Nlm4Share`.
- **std::marker::Copy**: Implemented for `FileSharingMode` and `FileSharingAccess`.
- **std::clone::Clone**: Implemented for `FileSharingMode`, `FileSharingAccess`, and `Nlm4Share`.
- **std::cmp::PartialEq**: Implemented for `FileSharingMode` and `FileSharingAccess`.

---

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here you MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to implement the "Share" (DOS compatibility) feature of the Network Lock Manager (NLM) version 4 protocol within the `nfs_mamont` NFS server. The system contains an NFS server that must support file locking not only via POSIX byte-range locks but also via DOS-style sharing modes, where a client can open a file with specific access rights (Read/Write) and explicitly deny other clients specific types of access. A typical usage scenario of the system involves a Windows client (or a client using DOS semantics) connecting to the NFS server and requesting to open a file. The client sends an NLM `SHARE` request containing its hostname, the file handle, and the desired access/deny modes. The system uses the `Nlm4Share` struct defined in this module to deserialize and validate this request. Inside the system, the following things happen and they use this module: The RPC handler extracts the raw arguments from the network packet and calls `Nlm4Share::new`. This constructor validates that the client's hostname (`caller_name`) is not empty and fits within the protocol-defined limit (`LM_MAXSTRLEN`). Once validated, the `Nlm4Share` instance is passed to the core lock manager logic, which compares the `fsh4_access` and `fsh4_mode` fields against existing shares and locks to determine if the request should be granted or denied. Without this module, the server would lack the structured representation of DOS share requests, making it impossible to correctly enforce compatibility sharing rules required for interoperability with certain operating systems.