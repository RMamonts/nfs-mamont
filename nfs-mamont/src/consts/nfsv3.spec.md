<!-- SPEC_HASH: fbd878270dc187a2772454d55497f22d02bb103e780b6a541b08ed1585302f0e -->
# Module Specification

Module: nfs_mamont::consts::nfsv3
Rust File: src/consts/nfsv3.rs

---

## 1. Dependencies

This module has no external dependencies. It relies solely on Rust primitive types (`u32`, `usize`) and contains no `use` statements.

---

## 2. Mechanics

This module acts as a static registry for the Network File System version 3 (NFSv3) protocol definitions. It does not contain executable logic or state transitions.

Intent:
- To define the fixed numerical identifiers required to implement the ONC RPC interface for NFSv3, ensuring compliance with RFC 1813.
- To provide compile-time constants for procedure numbers, program identifiers, and specific data structure sizes mandated by the protocol specification.

Inputs:
- None. The values are hard-coded literals.

Outputs:
- Public constants (`u32`, `usize`) representing protocol identifiers and sizes.

Steps:
- N/A (Static definitions).

Edge Cases:
- N/A.

Complexity:
- Time: O(1) (Compile-time resolution).
- Space: O(1) (Stored in static memory/compiled into binary).

Determinism:
- Deterministic.

---

## 3. Dependency Mechanics

There are no dependency mechanics to list as this module has no dependencies.

---

## 4. Data Model

Entities:
- **Protocol Identifiers**: Scalar values representing the NFS program number (`100003`) and version (`3`).
- **Procedure Numbers**: Scalar values mapping specific NFS operations (e.g., `READ`, `WRITE`, `CREATE`) to their unique protocol integers (1-21).
- **Size Constraints**: Scalar values defining fixed buffer sizes for protocol-specific opaque data types (File Handles, Verifiers).

Relations:
- N/A.

Global Invariants:
- The values defined here must match the IANA-assigned numbers and RFC 1813 specifications exactly for interoperability with standard NFS clients and servers.
- `NFS3_FHSIZE` is fixed at 64 bits (8 bytes) for NFSv3.

---

## 5. Error Model

Error Types:
- None.

Error Propagation Strategy:
- N/A.

Recoverability:
- N/A.

Panics:
- Allowed: No.
- Conditions: This module contains no executable code that could panic.

---

## 6. Traits

This module does not implement any external traits.

---

## 7. Overview

This module is used to establish the foundational "language" of the NFSv3 protocol within the `nfs_mamont` crate. It isolates the magic numbers defined in the NFSv3 specification (RFC 1813) into a single namespace, preventing "magic number" anti-patterns in the business logic of the server or client.

The system containing this module is an implementation of an NFSv3 server or client. In such a system, the RPC layer receives a request containing a procedure number. This module provides the constants against which that number is matched to determine which handler (e.g., read file, write file, lookup directory) to invoke. Furthermore, the size constants (e.g., `NFS3_FHSIZE`) are critical for memory allocation and serialization/deserialization logic, ensuring that the binary data exchanged over the network conforms to the expected 64-bit file handle and verifier sizes specific to version 3 of the protocol.

Without this module, the protocol logic would be littered with opaque integers, making the codebase difficult to maintain and verify against the standard. A typical usage scenario involves importing these constants to construct RPC headers or to pattern-match on incoming procedure identifiers. Inside the system, the serialization logic uses the size constants to define the length of byte arrays for file handles and verifiers, ensuring strict adherence to the protocol's wire format.