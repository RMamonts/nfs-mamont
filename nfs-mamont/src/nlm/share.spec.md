<!-- SPEC_HASH: c95dcf23a5ab342ee465fd22d76d9ce015d0d48cb6e95cf57b80c2f310f34429 -->
# Module Specification

Module: nfs_mamont::nlm::share
Rust File: src/nlm/share.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`std::io`**: Used to provide the `Error` and `ErrorKind` types. The `Nlm4Share::new` constructor returns `std::io::Result` to signal validation failures (e.g., empty or oversized caller names) using the `InvalidInput` error kind.
- **`crate::consts::nlm`**: Used to access the `LM_MAXSTRLEN` constant. This constant defines the maximum allowed length for the `caller_name` field, which is enforced during the construction of `Nlm4Share`.
- **`crate::vfs`**: Used to access the `vfs::file::Handle` type. This type is used within `Nlm4Share` to identify the specific file system object to which the sharing rules apply.
- **`super` (crate::nlm)**: Used to import `OpaqueHandle`. This type is used within `Nlm4Share` to identify the specific host or process (lock owner) that is requesting the share.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define the data structures representing NLMv4 file sharing arguments, specifically modeling DOS-style sharing semantics (compatibility modes).
- To enforce protocol-level validation on client identifiers (`caller_name`) at the boundary of the NLM service logic.

Inputs:
- **`Nlm4Share::new` arguments**:
 - `caller_name`: A `String` representing the hostname of the client.
 - `file_handle`: A `vfs::file::Handle` identifying the file.
 - `opaque_handle`: An `OpaqueHandle` identifying the lock owner.
 - `fsh4_mode`: A `FileSharingMode` enum variant defining restrictions for other clients.
 - `fsh4_access`: A `FileSharingAccess` enum variant defining permissions for the requesting client.

Outputs:
- **`Nlm4Share`**: A structure encapsulating the validated share request context.
- **`std::io::Error`**: Returned if validation of `caller_name` fails.

Steps:
1. **Enum Definition**: The module defines `FileSharingMode` and `FileSharingAccess` enums with explicit discriminants (0 through 3). These map directly to the integer values used in the NLMv4 wire protocol to represent "deny" modes (what others can't do) and "access" modes (what the requester can do).
2. **Validation (`Nlm4Share::new`)**:
 - Checks if `caller_name` is empty. If true, returns `Err(Error::new(InvalidInput, "caller_name must not be empty"))`.
 - Checks if `caller_name.len()` exceeds `nlm::LM_MAXSTRLEN`. If true, returns `Err(Error::new(InvalidInput, format!("caller_name is too long...")))`.
 - If checks pass, constructs the `Nlm4Share` struct with the provided fields and returns `Ok(Self)`.

Edge Cases:
- **Empty Caller Name**: Explicitly rejected to prevent invalid or anonymous share requests that cannot be tracked or managed correctly by the lock manager.
- **Oversized Caller Name**: Explicitly rejected to prevent buffer overflows or protocol violations, ensuring the string fits within the limits defined by `LM_MAXSTRLEN`.

Complexity:
- **Time**:
 - `Nlm4Share::new`: O(N) where N is the length of `caller_name` (due to the length check).
- **Space**:
 - `Nlm4Share`: O(N) where N is the length of `caller_name` (stored in the struct).

Determinism:
- Deterministic

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::consts::nlm`**:
 - **`LM_MAXSTRLEN`**: This constant provides the upper bound for the `caller_name` validation logic in `Nlm4Share::new`. It ensures that the module enforces the same size limits as the rest of the NLM protocol implementation.

- **From `nfs_mamont::vfs::file`**:
 - **`Handle`**: This structure is used as a key identifier within `Nlm4Share`. It allows the share module to reference specific files managed by the VFS layer without needing to know the internal implementation of the file system.

- **From `nfs_mamont::nlm` (parent module)**:
 - **`OpaqueHandle`**: This structure is used to identify the owner of the share request. It links the share request to the broader locking context managed by the NLM service, ensuring that share rules are associated with the correct client process/host.

---

## 4. Data Model

Entities:
- **`FileSharingMode`**: An enumeration defining what operations other clients are prohibited from performing.
 - Variants: `None` (0), `Read` (1), `Write` (2), `ReadWrite` (3).
- **`FileSharingAccess`**: An enumeration defining what operations the requesting client is allowed to perform.
 - Variants: `None` (0), `Read` (1), `Write` (2), `ReadWrite` (3).
- **`Nlm4Share`**: A structure representing a file share request.
 - Fields: `caller_name` (String), `file_handle` (vfs::file::Handle), `opaque_handle` (OpaqueHandle), `fsh4_mode` (FileSharingMode), `fsh4_access` (FileSharingAccess).

Relations:
- **Composition**: `Nlm4Share` *contains* instances of `FileSharingMode` and `FileSharingAccess`.
- **Association**: `Nlm4Share` *references* a `vfs::file::Handle` (identifying the file) and an `OpaqueHandle` (identifying the owner).

Global Invariants:
- **Caller Name Constraints**: For any instance of `Nlm4Share` created via `new`, the `caller_name` string length must be greater than 0 and less than or equal to `nlm::LM_MAXSTRLEN`.
- **Protocol Mapping**: The discriminant values of `FileSharingMode` and `FileSharingAccess` must match the NLMv4 protocol specification (0-3).

## 5. Error Model

Error Types:
- **`std::io::Error`**

Error Propagation Strategy:
- **Constructor Validation**: The module uses the "Fallible Constructor" pattern. `Nlm4Share::new` returns `std::io::Result<Self>`. If validation fails, it returns `Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "message"))`.

Recoverability:
- **Recoverable**: Callers (typically RPC handlers) can catch the `Err` result and map it to an appropriate NLM status code (e.g., `Nlm4Stats::Denied` or a generic failure) to be returned to the client, preventing the invalid request from affecting server state.

Panics:
- **Allowed**: No.
- **Conditions**: The public API performs explicit checks and returns `Result` types rather than panicking on invalid input.

---

## 6. Traits

List which external traits this module implements:
- **`Debug`**: Implemented for `FileSharingMode` and `FileSharingAccess`.
- **`Copy`**: Implemented for `FileSharingMode` and `FileSharingAccess`.
- **`Clone`**: Implemented for `FileSharingMode` and `FileSharingAccess`.
- **`PartialEq`**: Implemented for `FileSharingMode` and `FileSharingAccess`.

List which traits this module defines:
- None.

---

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than this level.

This module is used in order to **implement the DOS file sharing semantics required by the NLMv4 protocol**. While standard POSIX locking deals with read/write permissions, DOS sharing modes introduce a "deny" concept (e.g., "deny read" to others), which is necessary for compatibility with Windows clients and specific legacy applications that rely on these stricter locking behaviors. This module provides the data structures (`Nlm4Share`, `FileSharingMode`, `FileSharingAccess`) that allow the server to interpret and enforce these specific rules.

The system contains a complex NLM service that handles various locking procedures. This module specifically supports the `SHARE` and `UNSHARE` procedures (though only the share structure is explicitly defined here, it represents the state for both). A typical usage scenario involves a Windows client mounting an NFS share and opening a file with "exclusive" access. The client sends an NLM `SHARE` request containing a hostname, a file handle, and the desired sharing modes. The RPC layer deserializes this request and calls `Nlm4Share::new`. This constructor validates the hostname length against `LM_MAXSTRLEN`. If valid, the `Nlm4Share` object is passed to the lock manager, which checks if the requested `fsh4_access` (what the client wants) conflicts with existing `fsh4_mode` (what others are denied) on that specific `file_handle`.

Inside the system, the following things happen and they use this module:
1. **Protocol Translation**: The `FileSharingMode` and `FileSharingAccess` enums act as translators between the raw integer values on the wire (0-3) and semantic Rust types. This ensures that the internal logic operates on meaningful concepts (e.g., `ReadWrite`) rather than magic numbers.
2. **Validation Enforcement**: By centralizing the `caller_name` validation in `Nlm4Share::new`, the module ensures that no part of the lock manager needs to re-check these constraints. It acts as a gatekeeper, ensuring that only valid, protocol-compliant share requests propagate into the core locking logic.
3. **Context Association**: The `Nlm4Share` struct aggregates the `file_handle` (VFS identifier) and `opaque_handle` (NLM owner identifier). This links the abstract sharing rules to concrete resources and owners, allowing the lock manager to answer questions like "Does this specific client's request conflict with the deny modes set by this other specific client on this file?".

Without this module, the NLM service would lack the ability to process DOS sharing requests, breaking compatibility for clients that depend on this feature. It would also risk propagating malformed client identifiers (hostnames) deep into the locking logic, potentially causing inconsistencies or security issues.