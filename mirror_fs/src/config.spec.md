<!-- SPEC_HASH: e01123010e682b472070133dffa64e7780f468ffb7ce0eb3ae41a08fe3a1efe5 -->
# Module Specification

Module: mirrorfs::config
Rust File: src/config.rs

---

## 1. Dependencies

From the provided context, no internal project dependency specifications were listed. The module relies on the following external crates and standard library components:

- **`std`**:
    - `std::io`: For file reading operations (`read_to_string`) and error types (`std::io::Result`, `std::io::Error`).
    - `std::path`: For filesystem path manipulation, normalization, and component analysis.
    - `std::collections::HashSet`: Used internally for validating uniqueness of mount paths.
    - `std::num::NonZeroUsize`: For enforcing strictly positive integer constraints on configuration values at the type level.
- **`serde`**: Used for the `Deserialize` trait to facilitate parsing of the TOML configuration into intermediate structures (`RawConfig`, `RawAllocatorConfig`, etc.).
- **`toml`**: Used for deserializing TOML string data into Rust structures.

---

## 2. Mechanics

### Configuration Loading and Validation (`load_config`)

**Intent:**
To read a TOML configuration file from disk, deserialize it, and transform it into a strictly validated, runtime-ready `Config` object. This process ensures that resource allocation parameters are valid and that filesystem exports are safe to serve (non-overlapping, valid directories).

**Inputs:**
- `path`: A reference to a `Path` pointing to the TOML configuration file.

**Outputs:**
- `std::io::Result<Config>`: A populated `Config` struct on success, or an `std::io::Error` wrapping a specific validation message on failure.

**Steps:**
1.  **Read & Parse**: Reads the entire file content into a string and parses it using `toml::from_str` into `RawConfig`. If parsing fails, returns an `InvalidData` error.
2.  **Allocator Configuration**: Extracts allocator settings. If specific values are missing in the TOML, defaults are applied. It validates that all buffer sizes and counts are strictly greater than zero, converting them to `NonZeroUsize`.
3.  **VFS Pool Size**: Extracts the VFS pool size, applying a default if missing, and validates it is non-zero.
4.  **Export Section Validation**: Ensures the `[exports]` section exists and is not empty. Checks that the number of exports does not exceed `MAX_EXPORTS_COUNT` (256).
5.  **Path Resolution**:
    - Resolves the `export_root` defined in the config to an absolute, canonical path using `std::fs::canonicalize`. Verifies it is a directory.
    - Iterates through the list of export paths defined in the config.
    - Normalizes each export path to ensure it is relative and contains no `..` or `.` components.
    - Joins the normalized path with the resolved `export_root` to create the absolute `local_path`.
    - Generates a `mount_path` string (e.g., `/path/to/export`) from the normalized relative path.
6.  **Cross-Validation**:
    - **Uniqueness**: Ensures all generated `mount_path` strings are unique.
    - **Non-Overlapping**: Ensures no `local_path` is a prefix of another `local_path` (prevents nested exports which could cause namespace conflicts).
7.  **Construction**: Assembles the final `Config` struct with the validated allocator settings, pool size, root, and exports vector.

**Edge Cases:**
- Configuration file is unreadable or malformed TOML.
- Integer fields are zero or missing (where zero is invalid).
- Export paths contain `..` or are absolute.
- Export root does not exist or is not a directory.
- Duplicate mount paths are specified.
- Export directories overlap (e.g., exporting `/mnt/a` and `/mnt/a/b`).

**Complexity:**
- **Time**: O(N^2) in the worst case regarding the number of exports, due to the pairwise comparison for overlapping paths. Given the limit of 256 exports, this is acceptable.
- **Space**: O(N) for storing the list of exports and the HashSet for mount path validation.

**Determinism:**
- Deterministic. Given the same input file and filesystem state (regarding path resolution), the output is identical.

---

## 3. Dependency Mechanics

Since no internal dependency specifications were provided in the context, this section lists the mechanisms from standard library and external crates utilized by this module:

- **`std::fs::canonicalize`**: Used to resolve symlinks and relative paths to absolute paths. This is critical for ensuring that the `local_path` used for exports is unambiguous and refers to the specific inode intended.
- **`std::collections::HashSet`**: Used to enforce the invariant that mount paths must be unique within the configuration.
- **`toml::from_str`**: The mechanism responsible for transforming raw text bytes into the intermediate `RawConfig` structure, handling the syntax parsing logic.

---

## 4. Data Model

**Entities:**
- **`Config`**: The root configuration entity.
    - `allocator`: `AllocatorConfig` - Defines memory pooling parameters.
    - `vfs_pool_size`: `NonZeroUsize` - Size of the Virtual File System object pool.
    - `export_root`: `PathBuf` - The base directory on the host filesystem from which exports are calculated.
    - `exports`: `Vec<ExportConfig>` - The list of specific directories to export.
- **`AllocatorConfig`**: Defines buffer constraints.
    - `read_buffer_size`, `read_buffer_count`, `write_buffer_size`, `write_buffer_count`: All `NonZeroUsize`.
- **`ExportConfig`**: Maps a local directory to a virtual namespace.
    - `local_path`: `PathBuf` - Absolute, canonical path on the host.
    - `mount_path`: `String` - Virtual path string (e.g., "/export/name").

**Relations:**
- `Config` → `AllocatorConfig` (1:1)
- `Config` → `ExportConfig` (1:N)

**Global Invariants:**
- All `NonZeroUsize` fields are guaranteed to be > 0.
- `export_root` is an absolute, canonical path pointing to an existing directory.
- `exports` list is non-empty and contains at most 256 items.
- `mount_path` strings within `exports` are unique.
- `local_path` entries within `exports` do not overlap (none is a prefix of another).

---

## 5. Error Model

**Error Types:**
- `std::io::Error`: The module uses the standard IO error type for all failure conditions.

**Error Propagation Strategy:**
- Custom propagation. The module uses `std::io::Error::new` to wrap specific validation failures (mostly `std::io::ErrorKind::InvalidInput`) and IO failures (preserving the original `ErrorKind` from file operations).

**Recoverability:**
- Generally non-recoverable at the module level. If `load_config` returns an error, the `Config` cannot be constructed, implying the application should likely terminate or refuse to start rather than proceeding with invalid settings.

**Panics:**
- **Allowed**: No.
- **Conditions**: The public function `load_config` does not panic. The `Default` implementations use `unwrap()` on `NonZeroUsize::new`, but these are statically guaranteed to succeed because the constants (`DEFAULT_VFS_POOL_SIZE`, etc.) are non-zero.

---

## 6. Traits

The module implements the following standard traits for its public structs:
- **`std::fmt::Debug`**: Implemented for `Config`, `AllocatorConfig`, and `ExportConfig` (derived).
- **`std::default::Default`**: Implemented for `Config` and `AllocatorConfig`.

---

## 7. Overview

This module is used to initialize and validate the runtime environment of a file system serving application (likely a user-space NFS server or similar virtual file system). It acts as the gatekeeper between user-defined configuration and the system's internal logic.

The system contains a mechanism for serving local directories over a network protocol (exports) and a memory management subsystem (allocators) that relies on pre-allocated pools to guarantee performance and avoid runtime allocation failures.

A typical usage scenario involves the application starting up, reading a TOML file specified by the user, and invoking `load_config`. The module ensures that the user does not define conflicting exports (e.g., exporting the same directory twice or nesting exports) which could lead to ambiguous file lookups. It also ensures that memory pools are configured with valid sizes, preventing the system from attempting to allocate zero-sized buffers or empty pools.

Inside the system, the following things happen and they use this module:
1.  **Bootstrap**: The application calls `load_config` to translate static text into a structured `Config`.
2.  **Resource Reservation**: The `AllocatorConfig` is passed to memory allocators to pre-allocate read/write buffers, ensuring deterministic memory usage.
3.  **Namespace Construction**: The `exports` list is used to build the internal mapping table that translates incoming network requests (referencing `mount_path`) to local filesystem operations (referencing `local_path`).

Without this module, the system would lack a centralized, validated definition of its resources and namespace, increasing the risk of runtime errors due to misconfiguration.