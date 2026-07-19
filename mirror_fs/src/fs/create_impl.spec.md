<!-- SPEC_HASH: 5607db70b913c79731aac5a74ea708458c397273d0288314b8543ad26bce4443 -->
# Module Specification

Module: mirrorfs::fs::create_impl
Rust File: mirror_fs/src/fs/create_impl.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`tokio::fs::OpenOptions`**: Used to perform asynchronous file creation and opening operations on the local filesystem. It allows configuring specific flags such as `write(true)`, `create(true)`, and `create_new(true)` to implement the different NFS creation semantics (Unchecked, Guarded, Exclusive).
- **`nfs_mamont::vfs`**: Used to import the `vfs` module structure, specifically for accessing `vfs::Error` to map filesystem errors to NFS status codes and `vfs::WccData` to construct the Weak Cache Consistency data returned to the client.
- **`nfs_mamont::vfs::create`**: Used to import the `create` module traits and types. This module implements the `create::Create` trait for `MirrorFS`, utilizing `create::Args`, `create::Success`, `create::Fail`, and `create::How` to define the interface and handle the three distinct creation modes defined by the NFSv3 protocol.
- **`super::MirrorFS`**: The parent struct for which this implementation is provided. The implementation relies on `MirrorFS`'s internal state (implied by `self`) to map abstract file handles to concrete filesystem paths and vice versa.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

### Mechanism 1: Implementation of `create::Create` for `MirrorFS`

**Intent:**
To provide the concrete logic for creating regular files within the `MirrorFS` backend, translating the high-level NFSv3 `CREATE` procedure arguments into specific filesystem operations. This involves handling three distinct creation strategies (`Unchecked`, `Guarded`, `Exclusive`), managing file attributes, and ensuring Weak Cache Consistency (WCC) for the parent directory.

**Inputs:**
- `args: create::Args`: Contains the target directory (`object.dir`), the filename (`object.name`), and the creation strategy (`how`).

**Outputs:**
- `Result<create::Success, create::Fail>`: On success, returns the new file handle, its attributes, and WCC data. On failure, returns an error and WCC data.

**Steps:**
1. **Validation**: The filename is validated using `Self::ensure_name_allowed`. If invalid, an error is returned immediately.
2. **Path Resolution**: The directory handle is resolved to a filesystem path using `self.path_for_handle`. Metadata for this directory is retrieved to establish the "before" state for WCC.
3. **Directory Validation**: The implementation ensures the target path is a directory using `Self::validate_directory`.
4. **Existence Check**: The full path of the new file is constructed. `std::fs::symlink_metadata` is called to determine if the file already exists.
5. **Mode-Specific Execution**:
   - **Unchecked**: If the file does not exist, it is created using `OpenOptions::new().write(true).create(true).truncate(false)`. The attributes provided in `args.how` are then applied.
   - **Guarded**: If the file exists, the operation fails with `vfs::Error::Exist`. If it does not exist, the file is created using `OpenOptions::new().write(true).create_new(true)` (ensuring atomicity), and attributes are applied.
   - **Exclusive**: The implementation attempts to create the file atomically using `create_new(true)`.
     - If successful, a verifier is stored via `Self::store_exclusive_verifier`.
     - If the file already exists (`std::io::ErrorKind::AlreadyExists`), the implementation checks the existing verifier using `Self::check_exclusive_verifier`. If the verifier matches the request, the operation is treated as a success (idempotent retry). If it does not match, `vfs::Error::Exist` is returned.
     - Default attributes are applied in this mode.
6. **Finalization**: Metadata for the new file is retrieved, a handle is generated via `self.handle_for_path`, and WCC data for the directory is calculated.

**Edge Cases:**
- **Symlinks**: The existence check uses `symlink_metadata`, which follows symlinks. If a symlink exists at the target path pointing to a valid file, the creation logic (specifically in Guarded/Exclusive modes) will treat it as an existing file.
- **Idempotency**: In `Exclusive` mode, if the file already exists but the verifier matches, the operation succeeds without failing, allowing clients to retry requests safely.

**Complexity:**
- Time: O(1) relative to input size, dominated by filesystem I/O (syscalls).
- Space: O(1) (excluding path storage).

**Determinism:**
- Non-deterministic. The result depends on the current state of the filesystem (e.g., whether the file exists, disk availability).

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::create`**:
 - **`How` Enum**: The implementation explicitly branches on the variants of `How` (`Unchecked`, `Guarded`, `Exclusive`). The logic for handling file existence and attribute application is dictated by this enum.
 - **`Verifier`**: In `Exclusive` mode, the `Verifier` is passed to `Self::store_exclusive_verifier` and `Self::check_exclusive_verifier` to ensure the atomicity and uniqueness guarantees required by the protocol.

- **From `nfs_mamont::vfs`**:
 - **`WccData`**: The implementation constructs `WccData` using `Self::wcc_data` to return the pre- and post-operation attributes of the parent directory. This is critical for the client to validate its cache.
 - **`Error`**: Filesystem errors (e.g., `std::io::Error`) are converted into `vfs::Error` variants (like `Exist`, `IO`, `Access`) using `Self::io_error_to_vfs` to ensure the client receives standard NFS status codes.

---

## 4. Data Model

Entities:
- This module defines no new public entities. It operates on `create::Args`, `create::Success`, `create::Fail`, and the internal state of `MirrorFS`.

Relations:
- **Implementation**: `MirrorFS` implements `create::Create`.

Global Invariants:
- **Attribute Application**: In `Exclusive` mode, the code explicitly uses `&DEFAULT_SET_ATTR` instead of client-provided attributes, adhering to the NFSv3 specification which states that attributes should not be set on exclusive create (the verifier is the only data).
- **WCC Consistency**: The `before` WCC attribute is captured *before* any modification attempts, ensuring that even if the operation fails later, the client receives the correct pre-operation state.

## 5. Error Model

Error Types:
- **`vfs::Error`**: The primary error type returned within `create::Fail`. Specific variants include:
 - `Exist`: Returned in `Guarded` mode if the file exists, or in `Exclusive` mode if the verifier check fails.
 - `IO` / `Access` / `NoSpace`: Mapped from `std::io::Error` via `Self::io_error_to_vfs`.

Error Propagation Strategy:
- **Early Return**: The function uses a pattern of early returns wrapped in `Err(create::Fail { error, wcc_data })`. This ensures that WCC data is always available to the client, even on failure.

Recoverability:
- **`Exist`**: Indicates a logical conflict; retrying with the same arguments will fail again unless the file is deleted.
- **`IO`**: May be transient (e.g., network filesystem glitch) or permanent (disk failure).

Panics:
- **Allowed**: No explicit panics are present in the provided code. Errors are propagated via `Result`.

---

## 6. Traits

List which external traits this module implements:
- **`nfs_mamont::vfs::create::Create`**: The asynchronous trait defining the `create` method.

---

## 7. Overview

This module is used in order to **implement the file creation logic for the MirrorFS backend**, bridging the abstract NFSv3 `CREATE` procedure to the local filesystem. The system requires this module because the NFS `CREATE` operation is more complex than a simple "open file" syscall; it encompasses three distinct semantic modes (`Unchecked`, `Guarded`, `Exclusive`) that dictate how race conditions and existing files are handled.

The `MirrorFS` system acts as a local filesystem mirror, translating NFS requests into standard Rust/Tokio file operations. This module is necessary to ensure that these translations adhere strictly to the NFSv3 protocol. For example, the `Exclusive` mode requires a mechanism to verify that a client's retry attempt is creating the *same* file (using a `Verifier`) rather than a different one that happens to have the same name. This module handles that logic by abstracting the verifier storage/checking into helper methods (`store_exclusive_verifier`, `check_exclusive_verifier`).

Inside the system, the following things happen and they use this module:
1. **Protocol Enforcement**: When an NFS client requests a file creation, the RPC layer calls the `create` method on the `MirrorFS` instance. This module inspects the `how` argument to determine the correct strategy.
2. **Atomicity**: For `Guarded` and `Exclusive` modes, the module uses `create_new(true)` in `OpenOptions`. This leverages the OS kernel's atomicity guarantees to prevent race conditions where two processes might try to create the same file simultaneously.
3. **Cache Management**: By capturing directory metadata before and after the operation, this module populates the `WccData` structure. This allows the NFS client to update its directory cache efficiently without re-reading the entire directory listing, which is crucial for performance over the network.

**Uncertainty**: The helper methods `Self::ensure_name_allowed`, `Self::store_exclusive_verifier`, `Self::check_exclusive_verifier`, `Self::path_for_handle`, `Self::handle_for_path`, and `Self::apply_set_attr` are called but not defined in this file. They are assumed to be defined in the parent `MirrorFS` implementation or a utility module. The specific mechanism for storing the exclusive verifier (e.g., extended attributes, sidecar file) is abstracted behind these methods.