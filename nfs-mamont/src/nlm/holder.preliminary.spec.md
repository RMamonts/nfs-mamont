<!-- SPEC_HASH: 3ea5e9abb391d115ff3714c902f3f472d37b806031a5e829776bb07ca5051b6f -->
# Module Specification

Module: nfs_mamont::nlm::holder
Rust File: src/nlm/holder.rs

---

## 1. Dependencies

From `*.facts.json` and source analysis:

- **`super::OpaqueHandle`** (from `nfs_mamont::nlm`):
  - **Purpose:** Used to store the host or process specific identifier for the lock holder. It encapsulates a byte vector (`Vec<u8>`) representing an opaque identifier defined by the NLM protocol, allowing the system to carry arbitrary client-specific identification data.

---

## 2. Mechanics

**Intent:**
To provide a structured data container representing the owner of a Network Lock Manager (NLM) version 4 lock. This structure aggregates the lock's range (offset, length), type (exclusive/shared), and the identity of the holder (system ID, opaque handle) into a single pass-by-value or reference entity.

**Inputs:**
- `exclusive`: Boolean flag indicating if the lock is exclusive (`true`) or shared (`false`).
- `system_identifier`: 32-bit integer representing the Process ID (PID) of the holder.
- `opaque_handle`: Instance of `OpaqueHandle` containing the host or process specific identification bytes.
- `lock_offset`: 64-bit unsigned integer indicating the starting byte of the lock region.
- `lock_length`: 64-bit unsigned integer indicating the length of the lock region.

**Outputs:**
- An instance of `Nlm4Holder` containing the provided data.

**Steps:**
1. Accept the lock parameters and holder identity.
2. Initialize the `Nlm4Holder` struct fields with the provided values.
3. Return the initialized struct.

**Edge Cases:**
- The module does not enforce logic regarding the `lock_length` value of `0` (which semantically means "to end of file" in POSIX/NLM contexts); it merely stores the value.
- No validation is performed on the `system_identifier` or `opaque_handle` contents within this module.

**Complexity:**
- **Time:** O(1) (Struct initialization involves direct memory assignment/move).
- **Space:** O(1) regarding the struct metadata, plus the size of the heap-allocated data inside `OpaqueHandle`.

**Determinism:**
- Deterministic.

---

## 3. Dependency Mechanics

*Note: The specification for the parent module `nfs_mamont::nlm` was not provided. The following mechanics are derived strictly from the `*.facts.json` file for `nfs_mamont::nlm`.*

- **`OpaqueHandle::new(oh: Vec<u8>) -> io::Result<Self>`**:
  - Constructs a new `OpaqueHandle` from a vector of bytes. It returns an `io::Result`, indicating that construction may fail due to I/O-related errors or validation logic internal to that module.
- **`OpaqueHandle::as_bytes(&self) -> &[u8]`**:
  - Provides a read-only slice view of the underlying byte data stored in the handle. This is used to inspect the holder's identity without taking ownership.

---

## 4. Data Model

**Entities:**
- **`Nlm4Holder`**: A data structure representing a lock holder. It is a Plain Old Data (POD) struct containing metadata about the lock and the owner.

**Relations:**
- `Nlm4Holder` **owns** an `OpaqueHandle` (Composition).

**Global Invariants:**
- **Semantic Convention:** While not enforced by Rust type safety, a `lock_length` of `0` is conventionally treated by the broader NLM system as "lock to end of file".

---

## 5. Error Model

**Error Types:**
- None defined or generated within this module.

**Error Propagation Strategy:**
- N/A (This module defines a data structure and a constructor that does not return a `Result`).

**Recoverability:**
- N/A.

**Panics:**
- **Allowed:** No.
- **Conditions:** The code contains no explicit panic points or `unwrap()` calls in the public interface.

---

## 6. Traits

List which external traits this module implements:
- None.

---

## 7. Overview

This module is used in order to **standardize the representation of lock ownership** within the NLMv4 protocol implementation.

The system containing this module is a Network Lock Manager (NLM) server or client library that manages file locks across a network. In distributed file locking, it is critical to identify not just *what* is locked (the file region), but *who* holds the lock to resolve conflicts or grant blocking requests.

This module provides the `Nlm4Holder` struct, which serves as the primary data carrier for lock ownership information. It aggregates the process ID (`system_identifier`), a client-specific identifier (`opaque_handle`), and the lock's spatial properties (`offset`, `length`, `exclusive`).

A typical usage scenario of the system involves a lock request arriving at the server. The server checks its internal lock table. If a conflict is found, the server constructs a `Nlm4Holder` describing the conflicting lock and returns it to the client (e.g., in a `NLM4_DENIED` response). Inside the system, this structure allows the lock management logic to pass ownership details between the RPC layer (handling network packets) and the core state machine (managing lock tables) without being coupled to the specific wire format or internal storage mechanism.

**Assumptions:**
- The `OpaqueHandle` dependency is assumed to function as a wrapper for a byte vector, providing construction and read access, as inferred from the `*.facts.json` of the parent module. The specific validation logic inside `OpaqueHandle` is treated as a black box.