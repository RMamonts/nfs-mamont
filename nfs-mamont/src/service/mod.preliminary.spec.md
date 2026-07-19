<!-- SPEC_HASH: 3a06b0f09d68076883488df838375f6d55cf3707814b64a126b7c15a2b6f69eb -->
# Module Specification

Module: nfs_mamont::service
Rust File: src/service/mod.rs

---

## 1. Dependencies

- **`nfs_mamont::service::mount`**
  - **Purpose:** Provides the concrete server-side implementation for the MOUNT v3 protocol (`MountService`). It is used to handle client requests to mount file systems, requiring a list of export entries (`ExportEntryWrapper`) to initialize.
- **`nfs_mamont::service::nlm`**
  - **Purpose:** Provides the concrete server-side implementation for the NLM (Network Lock Manager) v4 protocol (`NlmService`). It is used to handle file locking requests over the network.

---

## 2. Mechanics

**Intent:**
The module serves as a top-level aggregation namespace for the service layer of the NFS stack. Its primary purpose is to organize and expose distinct RPC service implementations (MOUNT and NLM) under a unified `service` path, separating the server-side logic from protocol definitions or client logic.

**Inputs:**
- None directly at this module level.

**Outputs:**
- Public access to the `mount` submodule.
- Public access to the `nlm` submodule.

**Steps:**
1. The module declares the `mount` submodule as public, making `nfs_mamont::service::mount` accessible.
2. The module declares the `nlm` submodule as public, making `nfs_mamont::service::nlm` accessible.

**Edge Cases:**
- None. This is a structural module with no runtime logic.

**Complexity:**
- **Time:** N/A (Compile-time organization).
- **Space:** N/A.

**Determinism:**
- **Deterministic.**

---

## 3. Dependency Mechanics

The current module acts as a facade. The key mechanisms relevant to a user of this module are the constructors and configuration methods exposed by its submodules:

- **From `nfs_mamont::service::mount`:**
  - **`MountService::with_exports`**: A factory method used to instantiate the MOUNT service. It requires a vector of `ExportEntryWrapper`, which couples the service to specific file system handles and export configurations.
- **From `nfs_mamont::service::nlm`:**
  - **`NlmService::new`**: A default constructor for the NLM service, implying that the lock manager may not require complex external configuration to start.

---

## 4. Data Model

**Entities:**
- None defined directly in this module.

**Relations:**
- None defined directly in this module.

**Global Invariants:**
- None.

---

## 5. Error Model

**Error Types:**
- None defined in this module.

**Error Propagation Strategy:**
- N/A.

**Recoverability:**
- N/A.

**Panics:**
- **Allowed:** N/A.
- **Conditions:** N/A.

---

## 6. Traits

- **None.**
  - *Note:* The module documentation states that submodules contain handlers implementing protocol traits declared in higher-level modules (e.g., `crate::mount`). However, this module itself does not implement any traits.

---

## 7. Overview

This module is used in order to **isolate and organize the server-side RPC handlers** for the NFS stack. It provides a clear boundary between the abstract protocol definitions (likely defined in sibling or parent modules) and the concrete logic required to service network requests.

This system contains **the concrete implementations of the MOUNT v3 and NLM v4 protocols**. It encapsulates the state required to serve these protocols, such as the list of exported file systems in `MountService` and the locking state in `NlmService`.

A typical usage scenario of the system involves **instantiating the specific service structs** (e.g., `MountService::with_exports(...)`, `NlmService::new()`) and registering them with an RPC server runtime (e.g., using `tarpc` or a similar framework). The external runtime invokes methods on these structs when network requests arrive.

Inside the system the following things happen and they use **the submodules to delegate protocol-specific logic**. The `service` module itself does not process data; it merely exposes the `mount` and `nlm` modules, which contain the actual structs (`MountService`, `NlmService`) that implement the heavy lifting of parsing RPC arguments and interacting with the underlying file system or lock manager.

**Uncertainty:**
The specific traits that `MountService` and `NlmService` implement are not listed in the provided `*.facts.json` files, though the documentation implies they implement traits defined in higher-level modules (e.g., `crate::mount`). The specific RPC framework used (e.g., `tarpc`, `gRPC`) is also not specified in the provided code.