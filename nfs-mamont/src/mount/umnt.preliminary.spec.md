<!-- SPEC_HASH: 59997be779fc1744ee4890b10b2140252e49d046c490471290449959cddfa2ae -->
# Module Specification

Module: nfs_mamont::mount::umnt
Rust File: src/mount/umnt.rs

---

## 1. Dependencies

- **`std::net::SocketAddr`**
  - **Purpose:** Used to identify the network endpoint (IP address and port) of the client requesting the unmount operation. This is necessary because the MOUNT protocol tracks mounts per client IP.

- **`crate::vfs::file`**
  - **Purpose:** Provides the `Path` type used to encapsulate the directory path string. Based on the dependency facts, `Path` acts as a validated wrapper around a string, ensuring that the `dirpath` argument conforms to expected filesystem path constraints before being processed.

- **`trait_variant::make`**
  - **Purpose:** A macro used to transform the `Umnt` trait definition. It generates an object-safe version of the trait and automatically implements the `Send` marker trait. This allows the `Umnt` trait to be used as a trait object (e.g., `dyn Umnt`) in asynchronous contexts where thread safety is required.

---

## 2. Mechanics

**Intent:**
To define the server-side interface for the MOUNT protocol version 3 "UMNT" procedure (Procedure 3, as per RFC 1813). This interface allows a client to explicitly notify the server that it has finished using a specific directory, enabling the server to release resources associated with that specific client-mount pair.

**Inputs:**
- `args: Args`: A structure containing the `dirpath` (type `file::Path`) representing the server-side directory to be unmounted.
- `client_addr: SocketAddr`: The network address of the client issuing the request.

**Outputs:**
- `()`: The function returns nothing. According to RFC 1813, there are no MOUNT protocol errors returned from this procedure.

**Steps:**
1. The implementation receives the `Args` struct and the `client_addr`.
2. The implementation locates the mount table entry corresponding to the provided `dirpath` and `client_addr`.
3. The implementation removes the entry from the mount table.
4. If no such entry exists, the operation effectively does nothing (idempotency).
5. The operation completes successfully without returning a value to the caller.

**Edge Cases:**
- **Non-existent mount:** If the client attempts to unmount a path that it has not currently mounted, or if the path does not exist, the procedure must not return an error. The server should simply ensure the entry is absent.
- **Invalid path:** While the `file::Path` type implies validation, the `umnt` procedure itself assumes a valid `Path` object is passed.

**Complexity:**
- **Time:** Dependent on the underlying data structure used to store mount entries (not defined in this module), typically O(1) or O(log N) for lookup and deletion.
- **Space:** O(1) for the arguments passed; the operation may free space in the server's mount table.

**Determinism:**
- **Deterministic**

---

## 3. Dependency Mechanics

- **`nfs_mamont::vfs::file::Path`**
  - **Validation:** The `Path` struct enforces validity of the directory string. The `umnt` interface relies on `Path` to ensure that the `dirpath` is a correctly formatted filesystem path before the unmount logic is executed.
  - **Access:** The `umnt` logic will likely need to inspect the internal string representation of the `Path` (via `as_path` or `into_inner`) to perform comparisons against stored mount paths.

---

## 4. Data Model

**Entities:**
- **`Args`**
  - A data transfer object (DTO) holding the arguments for the unmount operation.
  - Contains `dirpath`: The server pathname of the directory to be unmounted.

- **`Umnt` (Trait)**
  - An abstract interface defining the behavior required to process an unmount request.
  - It is marked as `Send`, allowing implementations to be shared across threads.

**Relations:**
- `Args` → `file::Path` (Composition): `Args` owns a `file::Path`.

**Global Invariants:**
- None defined within this module.

---

## 5. Error Model

**Error Types:**
- None. The `umnt` function signature returns `()`.

**Error Propagation Strategy:**
- **Silent Success / Idempotency:** The interface explicitly forbids returning errors to the RPC client. Any internal issues (e.g., failure to access the mount table) must be handled internally (e.g., logged) or result in a panic, but cannot be propagated as a return value.

**Recoverability:**
- N/A (No errors are returned).

**Panics:**
- **Allowed:** Yes.
- **Conditions:** If the implementation encounters an unrecoverable internal state (e.g., lock poisoning in the mount table), it may panic. However, the interface design suggests a "best effort" approach where standard operational errors are ignored.

---

## 6. Traits

- **`Umnt`**: Defined in this module. It is an asynchronous trait (`async fn umnt`) made `Send` via the `trait_variant` macro.

---

## 7. Overview

This module is used in order to define the contract for the "Unmount" operation within the NFS MOUNT protocol version 3. It serves as the boundary between the network RPC layer (which receives the request bytes) and the server's internal state management (the mount table).

The system containing this module is an NFS server implementation. The MOUNT protocol is a auxiliary protocol used by NFS clients to obtain file handles for directories and to manage the lifecycle of these mounts. The `Umnt` trait specifically addresses the cleanup phase of this lifecycle.

A typical usage scenario involves a client shutting down or explicitly unmounting a filesystem. The client sends a UMNT request to the server. The server, acting as an implementor of the `Umnt` trait, receives the `Args` (containing the path) and the `client_addr`. The server then updates its internal records to reflect that this specific client no longer requires access to this directory. This is critical for preventing resource leaks (e.g., keeping track of stale clients) and for maintaining an accurate view of network activity.

Inside the system, the following things happen: The RPC handler decodes the request, constructs the `Args` struct, and invokes the `umnt` method on a service object implementing this trait. The implementation uses the `client_addr` to distinguish between different clients that may have mounted the same path. The `file::Path` ensures that the path string is handled in a type-safe, validated manner consistent with the rest of the `nfs_mamont` VFS layer. The absence of a return value enforces the protocol requirement that the server must not reject an unmount request, even if the mount entry does not exist.