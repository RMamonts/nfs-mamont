<!-- SPEC_HASH: 5ab2fb671884ca5d34bfc15df89ff7217cef8fa4906d30e7defc84f0ae02076b -->
# Module Specification

Module: nfs_mamont::consts::mount
Rust File: src/consts/mount.rs

---

## 1. Dependencies

This module has no dependencies on other external crates or internal modules. It relies exclusively on Rust built-in primitive types (`usize`, `u32`) to define constant values.

---

## 2. Mechanics

This module acts as a static configuration and registry of protocol identifiers. It does not implement runtime logic or state transitions.

Intent:
- To define the numerical constants required for interoperability with the NFS MOUNT protocol (Version 3).
- To enforce protocol-defined size constraints on filesystem path and hostname strings.

Inputs:
- None (Compile-time constants).

Outputs:
- Public constant bindings representing RPC program numbers, procedure IDs, and buffer size limits.

Steps:
1. Define the maximum allowed length for directory paths (`MOUNT_DIRPATH_LEN`) and host names (`MOUNT_HOST_NAME_LEN`) according to protocol limits.
2. Define the official RPC program number (`MOUNT_PROGRAM`) and version number (`MOUNT_VERSION`) assigned to the MOUNT service.
3. Define the procedure numbers (`MOUNT_NULL` through `MOUNT_EXPORT`) corresponding to the specific RPC methods available in the MOUNT protocol.

Edge Cases:
- Not applicable (static data).

Complexity:
- Time: O(1) (Compile-time resolution).
- Space: O(1) (Stored in static memory).

Determinism:
- Deterministic

---

## 3. Dependency Mechanics

Not applicable. The module has no dependencies.

---

## 4. Data Model

Entities:
- **Protocol Identifiers**: Scalar values (`u32`) representing the unique IDs for the MOUNT program, its version, and its specific procedures (e.g., `MOUNT_MNT`, `MOUNT_UMNT`).
- **Size Constraints**: Scalar values (`usize`) defining the maximum byte length for path strings and hostname strings.

Relations:
- None.

Global Invariants:
- The values for `MOUNT_PROGRAM` and `MOUNT_VERSION` must match the standard definitions defined by IETF RFCs (specifically RFC 1813 for NFSv3) to ensure compatibility with standard NFS clients and servers.
- Size limits (`MOUNT_DIRPATH_LEN`, `MOUNT_HOST_NAME_LEN`) must be respected by any encoding/decoding logic utilizing these constants to prevent buffer overflows or protocol violations.

---

## 5. Error Model

Error Types:
- None defined in this module.

Error Propagation Strategy:
- Not applicable.

Recoverability:
- Not applicable.

Panics:
- Allowed: No
- Conditions: This module contains only constant definitions; no code execution occurs that could result in a panic.

---

## 6. Traits

This module does not implement any external traits.

---

## 7. Overview

This module is used in order to centralize the "magic numbers" required for the implementation of the NFS MOUNT protocol within the `nfs_mamont` system. The MOUNT protocol is a distinct auxiliary protocol used by NFS to map a server-side file system path to a generic file handle, which is then used by the main NFS protocol for file operations.

This system contains a set of constant definitions that serve as the contract between the `nfs_mamont` implementation and the external NFS ecosystem. By isolating these values, the system ensures that RPC dispatchers, client stubs, and server handlers all refer to the same source of truth for procedure numbers and protocol versions.

A typical usage scenario of the system involves an RPC dispatcher receiving a request. It checks the program number against `MOUNT_PROGRAM` and the version against `MOUNT_VERSION`. If they match, it inspects the procedure number (e.g., `MOUNT_MNT`) to route the request to the appropriate handler. The handler, in turn, uses `MOUNT_DIRPATH_LEN` to validate that the path provided by the client does not exceed the maximum allowed size before processing it.

Inside the system, the following things happen and they use these constants to ensure strict adherence to the NFS MOUNT v3 specification, preventing miscommunication with heterogeneous NFS clients.