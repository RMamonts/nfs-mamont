<!-- SPEC_HASH: b4ff4a83916877b54ed5e7209c21de645b8e1ec5ec65680595cb1e89d22f5d39 -->
# Module Specification

Module: nfs_mamont::service::mount::export
Rust File: src/service/mount/export.rs

---

## 1. Dependencies

- **`crate::mount::export::{Export, Success}`**
    - **Purpose:** Used to define the trait `Export` which is being implemented for `MountService`, and the struct `Success` which serves as the return type for the procedure.
- **`super::MountService`**
    - **Purpose:** The concrete service struct for which the `Export` trait is implemented. This struct holds the runtime state required to serve the request.

---

## 2. Mechanics

**Intent:**
To provide the concrete implementation of the MOUNT v3 `EXPORT` procedure for the `MountService`. This module acts as the adapter between the generic protocol interface defined in `crate::mount::export` and the specific internal state management of `MountService`.

**Inputs:**
- `&self`: A reference to the `MountService` instance.

**Outputs:**
- `Success`: A struct containing a vector of `ExportEntry` items representing the exported file systems.

**Steps:**
1. The `export` method is invoked on the `MountService` instance via the `Export` trait.
2. The implementation accesses the `exports` field of `MountService`.
3. It calls the `export_list()` method on this field to retrieve the current list of exported directories.
4. The retrieved list is wrapped in the `Success` struct.
5. The `Success` struct is returned to the caller.

**Edge Cases:**
- **Empty State:** If the internal `exports` field contains no entries, `export_list()` returns an empty vector, resulting in a `Success` struct with an empty list.

**Complexity:**
- **Time:** Dependent on the complexity of `self.exports.export_list()`.
- **Space:** O(N), where N is the number of exported file systems returned by `export_list()`.

**Determinism:**
- Deterministic (assuming the behavior of `self.exports.export_list()` is deterministic).

---

## 3. Dependency Mechanics

- **`crate::mount::export::Export`**:
    - Defines the asynchronous interface `async fn export(&self) -> Success`. The current module fulfills this contract.
- **`crate::mount::export::Success`**:
    - Defines the structure of the successful response, specifically requiring a field `exports: Vec<ExportEntry>`.
- **`super::MountService`**:
    - **Assumption:** Although the public fields of `MountService` are not explicitly listed in the provided facts for the parent module, the code `self.exports.export_list()` implies that `MountService` possesses a field named `exports`. This field must have a method `export_list()` that returns a `Vec<ExportEntry>` (or a compatible type convertible to the `exports` field of `Success`).

---

## 4. Data Model

**Entities:**
- No new entities are defined in this module. It utilizes `Success` and `ExportEntry` from dependencies.

**Relations:**
- N/A (No new relations defined).

**Global Invariants:**
- The implementation ensures that `MountService` satisfies the `Export` trait contract, specifically that it always returns a `Success` struct (infallible at the protocol level).

---

## 5. Error Model

**Error Types:**
- None defined in this module. The implementation signature matches the trait definition which does not return a `Result`.

**Error Propagation Strategy:**
- N/A.

**Recoverability:**
- N/A.

**Panics:**
- **Allowed:** No specific constraints are defined in the code.
- **Conditions:** Potential panic if `self.exports` is uninitialized or if `export_list()` panics, though these are external to this specific logic block.

---

## 6. Traits

- **`crate::mount::export::Export`** (implemented for `MountService`)

---

## 7. Overview

This module is used in order to **bridge the abstract MOUNT protocol definition with the concrete service implementation**, enabling the `MountService` to respond to `EXPORT` RPC requests.

This system contains **the separation of concerns between the protocol logic (defined in `nfs_mamont::mount`) and the service application logic (defined in `nfs_mamont::service`)**. The protocol layer defines *what* an export response looks like (`Success`), while this module defines *how* the specific service constructs that response from its internal state.

A typical usage scenario of the system involves an NFS client requesting the list of exported file systems from the server. The server's RPC dispatcher, holding a reference to the `MountService`, calls the `export` method. This module executes, retrieving the data from the service's internal storage (`self.exports`) and packaging it into the protocol-standard `Success` struct.

Inside the system the following things happen and they use **the `MountService` as the central state holder**. The `MountService` delegates the actual retrieval of export data to an internal component (accessed via `self.exports`). This module serves as the glue code that connects the service's internal data retrieval mechanism (`export_list()`) to the external protocol interface (`Export`), ensuring that the server can communicate its export list to clients in the format specified by RFC 1813.