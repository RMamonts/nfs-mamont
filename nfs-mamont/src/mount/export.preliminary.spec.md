<!-- SPEC_HASH: bc5abdd30064167ed54e65ce5d93198f85519d72b35c913ece5fa3ffc91bed82 -->
# Module Specification

Module: nfs_mamont::mount::export
Rust File: src/mount/export.rs

---

## 1. Dependencies

- **`super::ExportEntry`**
  - **Purpose:** Used as the item type within the `Vec` contained in the `Success` struct. It defines the structure of a single export record (directory path and allowed client hostnames).
- **`trait_variant::make`**
  - **Purpose:** A macro used to generate a `Send` version of the `Export` trait. This allows the trait to be converted into a trait object (`dyn Export + Send`), facilitating dynamic dispatch across thread boundaries in asynchronous contexts.

---

## 2. Mechanics

**Intent:**
To define the asynchronous interface for the MOUNT protocol version 3, Procedure 5 (EXPORT), as specified in RFC 1813. This interface allows a server to communicate the list of exported file systems and the specific clients permitted to access them.

**Inputs:**
- `&self`: A reference to the implementor of the trait.

**Outputs:**
- `Success`: A struct containing a vector of `ExportEntry` items.

**Steps:**
1. The consumer invokes the `export` method on an object implementing the `Export` trait.
2. The implementation asynchronously retrieves the current configuration of exported file systems.
3. The retrieved data is mapped into a `Vec<ExportEntry>`.
4. The vector is wrapped in the `Success` struct and returned to the caller.

**Edge Cases:**
- **Empty Export List:** If no file systems are currently exported, the `exports` field in the `Success` struct will be an empty vector.
- **Protocol Errors:** According to RFC 1813, this procedure does not return MOUNT protocol errors. The Rust signature enforces this by returning `Success` directly rather than a `Result` type.

**Complexity:**
- **Time:** Dependent on the implementation of the trait (specifically, the time required to gather the list of exports).
- **Space:** O(N), where N is the number of exported file systems, required to store the `Vec<ExportEntry>`.

**Determinism:**
- Deterministic (assuming the underlying storage mechanism for export configurations is deterministic).

---

## 3. Dependency Mechanics

From the `nfs_mamont::mount` module (parent module):

- **`ExportEntry` struct**: Represents the atomic unit of the export list. It couples a `file::Path` (the exported directory) with a `Vec<HostName>` (list of allowed clients). This is the primary data carrier for the `Success` struct defined in the current module.
- **`MountRes` enum**: Contains a variant `Export(export::Success)`. This indicates that the `Success` struct defined in this module is intended to be wrapped in this enum when returned as part of the broader MOUNT protocol dispatch logic.

---

## 4. Data Model

**Entities:**
- **`Success`**: A container struct representing the successful response of the EXPORT procedure.
  - `exports`: `Vec<ExportEntry>` — The list of all exported directories and their access controls.

**Relations:**
- `Success` 1:N `ExportEntry` — A single success response contains zero or more export entries.

**Global Invariants:**
- The `Export` trait is object-safe and `Send`, meaning any implementation must be safe to transfer across thread boundaries.
- The `export` method is infallible regarding MOUNT protocol errors; it always returns a `Success` struct.

---

## 5. Error Model

**Error Types:**
- None defined in the public interface. The method signature `async fn export(&self) -> Success` indicates that no MOUNT protocol-specific errors are expected to be returned via this interface.

**Error Propagation Strategy:**
- N/A (The interface does not return a `Result`).

**Recoverability:**
- N/A.

**Panics:**
- **Allowed:** No specific constraints are defined in the code, but standard Rust async practices suggest avoiding panics.
- **Conditions:** None specified.

---

## 6. Traits

- **`Export`**: The primary trait defined in this module. It requires an asynchronous `export` method. It is marked as `Send` via the `trait_variant` macro.

---

## 7. Overview

This module is used in order to **implement the server-side interface for the NFS MOUNT protocol's "Export" procedure**.

This system contains **the abstraction layer for the MOUNT protocol version 3 (RFC 1813)**. The MOUNT protocol is distinct from the NFS protocol itself; it is responsible for resolving directory pathnames to file handles and managing access control lists before NFS operations can proceed.

A typical usage scenario of the system involves a client connecting to the server and requesting the list of available shares without prior knowledge of the server's directory structure. The client invokes the Export procedure. The server, implementing the `Export` trait defined in this module, retrieves the internal list of exported directories (defined as `ExportEntry` in the parent module) and returns them wrapped in the `Success` struct.

Inside the system the following things happen and they use **the `Export` trait to decouple the RPC handling logic (which likely uses the `MountRes` enum from the parent module) from the actual storage or configuration backend that holds the export list**. The `Success` struct serves as the data transfer object (DTO) that bridges the backend logic and the network encoding layer. The explicit lack of error return types in the signature enforces the RFC requirement that this specific procedure cannot fail at the protocol level.