<!-- SPEC_HASH: e4857555f775038b75647ea17e62c49fb4fadbf3cdfba9a4ab8593d23ecd5284 -->
# Module Specification

Module: nfs_mamont::vfs::rename
Rust File: src/vfs/rename.rs

---

## 1. Dependencies

- **`crate::vfs`**
  - **Purpose:** Provides the core data structures required to define the interface. Specifically, it supplies `vfs::DirOpArgs` for identifying source and target directories, `vfs::WccData` for weak cache consistency information, and `vfs::Error` for standardized error reporting across the VFS layer.
- **`trait_variant`**
  - **Purpose:** Used to generate a `Send`-compatible version of the `Rename` trait. This allows the trait to be used as a trait object (`dyn Rename`) in multi-threaded, asynchronous contexts, which is essential for an NFS server implementation.

---

## 2. Mechanics

**Intent:**
To define the contract for the NFSv3 `RENAME` procedure within the VFS layer. This contract enforces atomicity, handles cross-device restrictions, manages target overwriting logic, and ensures that cache consistency data (WCC) is returned for both the source and target directories.

**Inputs:**
- `args: Args`: A structure containing:
  - `from`: `vfs::DirOpArgs` identifying the source directory handle and the current name of the object.
  - `to`: `vfs::DirOpArgs` identifying the target directory handle and the new name for the object.

**Outputs:**
- `Result<Success, Fail>`:
  - `Success`: Contains `from_dir_wcc` and `to_dir_wcc` (Weak Cache Consistency data) indicating the state of the directories before and after the operation.
  - `Fail`: Contains a `vfs::Error` describing the failure, along with `from_dir_wcc` and `to_dir_wcc`.

**Steps:**
1. **Validation:** Verify that `from` and `to` reside on the same file system (check `fsid`). If not, return `Error::XDev`.
2. **Name Sanity Check:** Ensure filenames are not "." or ".." and are not aliases for the directory itself. If invalid, return `Error::InvalidArgument`.
3. **Identity Check:** If `from` and `to` refer to the same file (e.g., hard links), perform no action and return `Success`.
4. **Target Existence Check:** If the target name exists in the target directory:
   - Verify compatibility (non-directory to non-directory, or directory to empty directory).
   - If incompatible or target is a non-empty directory, return `Error::Exist`.
   - If compatible, prepare to remove the existing target.
5. **Execution:** Perform the atomic rename operation.
6. **WCC Collection:** Capture the pre-operation (before) and post-operation (after) attributes for both the source and target directories.
7. **Response:** Return `Success` or `Fail` populated with the collected WCC data.

**Edge Cases:**
- **Cross-device rename:** Attempting to rename across different file systems results in `Error::XDev`.
- **Self-rename:** Renaming a file to itself (or a hardlink to itself) is a no-op but returns success.
- **Overwriting non-empty directory:** Attempting to rename a file over a non-empty directory results in `Error::Exist`.
- **Internal implementation details:** The operation may internally use an unlink/link/unlink sequence, which could theoretically trigger `Error::TooManyLinks` even if the operation is logically atomic.

**Complexity:**
- **Time:** Dependent on the underlying file system implementation, but expected to be efficient (typically metadata updates).
- **Space:** O(1) for the structures returned.

**Determinism:**
- **Deterministic:** Given the same state of the file system and inputs, the output (Success/Fail and WCC data) is deterministic.

---

## 3. Dependency Mechanics

Based on the `*.facts.json` for `vfs::mod`, the following mechanisms are critical:

- **`vfs::DirOpArgs`**:
  - Acts as the primary identifier for directory operations, bundling a `file::Handle` (directory) and a `file::Name` (entry).
- **`vfs::WccData`**:
  - Encapsulates `before` and `after` attributes. This is crucial for the NFS protocol to allow clients to validate cached data without re-reading the entire directory.
- **`vfs::Error`**:
  - A comprehensive enum defining specific error conditions (e.g., `XDev`, `Exist`, `InvalidArgument`, `TooManyLinks`) that map directly to NFSv3 status codes.

---

## 4. Data Model

**Entities:**
- **`Args`**: Input parameters for the rename operation.
  - `from`: Source location.
  - `to`: Target location.
- **`Success`**: Successful result wrapper.
  - `from_dir_wcc`: Cache data for the source directory.
  - `to_dir_wcc`: Cache data for the target directory.
- **`Fail`**: Failure result wrapper.
  - `error`: The specific error encountered.
  - `from_dir_wcc`: Cache data for the source directory (even on failure).
  - `to_dir_wcc`: Cache data for the target directory (even on failure).

**Relations:**
- `Args` **aggregates** two `vfs::DirOpArgs`.
- `Success` and `Fail` **aggregate** two `vfs::WccData` instances.

**Global Invariants:**
- **Atomicity:** The rename operation must appear atomic to the client.
- **WCC Availability:** Both `Success` and `Fail` results must always contain WCC data for both directories involved.

---

## 5. Error Model

**Error Types:**
- **`vfs::Error`**: Enumerated errors defined in the parent `vfs` module.
  - `XDev`: Cross-device link attempt.
  - `Exist`: File exists or target directory not empty.
  - `InvalidArgument`: Invalid name (e.g., ".", "..") or argument aliasing.
  - `TooManyLinks`: Internal link limit reached.
  - (Other standard VFS errors may apply depending on implementation).

**Error Propagation Strategy:**
- **Custom Enum**: Errors are returned as a specific variant of `vfs::Error` wrapped in the `Fail` struct. This allows the caller to distinguish between different failure modes specific to the rename operation.

**Recoverability:**
- Errors returned via `Fail` indicate the operation did not complete. The client must interpret the error code and WCC data to update its state. The operation itself is not automatically retriable by the trait; the caller decides retry logic.

**Panics:**
- **Allowed**: No.
- **Conditions**: The trait definition implies a robust implementation where panics should not occur for standard input validation or file system states. Errors should be returned via the `Result` type.

---

## 6. Traits

- **`Rename`**: The primary trait defined by this module. It is made `Send` via the `trait_variant` macro.
  - **Method**: `rename(&self, args: Args) -> Result<Success, Fail>`.

---

## 7. Overview

This module is used in order to define the interface for moving or renaming file system objects within an NFSv3 server implementation. It abstracts the complexities of the rename operation—specifically the requirements for atomicity, cross-device restrictions, and cache consistency—into a single asynchronous trait method.

This system contains a Virtual File System (VFS) layer that acts as an abstraction between the raw network protocol handling and the specific backend storage. The `vfs::rename` module is a specific component of this layer responsible for the `RENAME` procedure.

A typical usage scenario of the system involves an NFS client requesting to rename a file. The server decodes this request into `Args` (source and target directory handles and names). The server then invokes the `rename` method on the VFS implementation. The VFS implementation performs the necessary checks (filesystem IDs, name validity, target compatibility) and executes the rename. Crucially, it gathers `WccData` for both directories before and after the operation. This data is returned to the client, allowing the client to update its cache without performing expensive `READDIR` operations, ensuring efficient synchronization of the distributed file system state.

Inside the system the following things happen and they use the `vfs::rename` module to enforce protocol compliance:
1.  **Request Handling**: The server receives a rename request.
2.  **Abstraction**: The request is mapped to the `Rename` trait.
3.  **Execution**: The backend implementation performs the file system modification.
4.  **Consistency**: `WccData` is captured to reflect changes to directory attributes (modification time, etc.) resulting from the rename.
5.  **Response**: The result (Success or Fail) is serialized back to the NFS client.

**Assumptions:**
- Types `file::Handle`, `file::Name`, `file::WccAttr`, and `file::Attr` (referenced in `vfs::DirOpArgs` and `vfs::WccData`) are opaque identifiers and attribute structures defined in a sibling `file` module within `vfs`. Their exact internal structure is not needed to understand this module, which treats them as data carriers.
- The `trait_variant` crate functions as a standard macro to generate `Send` trait objects.