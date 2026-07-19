<!-- SPEC_HASH: 8090f15e7d53e0f5bb059bff843f8a671a7ed497ffde66d3e5b2fd415bac389b -->
# Module Specification

Module: nfs_mamont::parser::nfsv3::create
Rust File: src/parser/nfsv3/create.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`std::io::Read`**: Used as the generic source trait for the `src` parameter in all parsing functions. It allows the parser to consume bytes from network streams or test buffers.
- **`crate::parser::nfsv3::file`**: Used to parse the directory handle (`file::handle`) and the file name (`file::file_name`) which constitute the `object` field of the `CREATE` arguments.
- **`crate::parser::primitive`**: Used to perform low-level XDR (External Data Representation) parsing. Specifically, `u32` and `u64` are used for integers and discriminants, `option` is used to parse optional attribute fields in `new_attr`, and `array` is used to parse the fixed-size `Verifier` in the `Exclusive` creation mode.
- **`crate::parser`**: Used to import the `Error` enum and `Result` type alias. This allows the module to report parsing failures (e.g., `EnumDiscMismatch`) consistently with the rest of the parser subsystem.
- **`crate::vfs::create`**: Used to import the target data structures (`Args`, `How`, `Verifier`) that the parsing functions populate. These structures are the canonical representation of `CREATE` arguments passed to the VFS layer.
- **`crate::vfs::set_attr`**: Used to import `NewAttr` and `SetTime`. These structures are embedded within the `create::How` enum (for `Unchecked` and `Guarded` modes) and define the attributes to be set during file creation.
- **`crate::vfs::file`**: Used to import the `Time` structure, which is used to represent timestamps within the `SetTime` enum.
- **`crate::consts::nfsv3`**: Used to import `NFS3_CREATEVERFSIZE`. This constant defines the size of the `Verifier` array (8 bytes) used in the `Exclusive` creation mode, ensuring compliance with the NFSv3 protocol.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To deserialize the arguments for the NFSv3 `CREATE` procedure from a byte stream into the high-level `vfs::create::Args` structure.
- To handle the conditional parsing logic required by the `createhow3` union (mapped to `vfs::create::How`), which switches between parsing attribute structures (`sattr3`) or a verifier (`createverf3`) based on a discriminant.
- To parse the `sattr3` structure (mapped to `vfs::set_attr::NewAttr`), which consists of optional fields for mode, ownership, size, and timestamps.

Inputs:
- `src: &mut impl Read`: A mutable reference to a byte stream (e.g., network socket or test cursor) from which XDR-encoded data is read.

Outputs:
- `Result<create::Args>`: The fully parsed arguments for the CREATE procedure.
- `Result<create::How>`: The parsed creation mode enum.
- `Result<set_attr::NewAttr>`: The parsed attribute set structure.
- `Result<set_attr::SetTime>`: The parsed timestamp setting strategy.
- `Result<file::Time>`: The parsed timestamp value.

Steps:
1. **`args` function**:
 - Calls `file::handle(src)` to read the directory file handle.
 - Calls `file::file_name(src)` to read the name of the file to create.
 - Calls `how(src)` to read the creation mode and associated data.
 - Constructs and returns `create::Args`.
2. **`how` function**:
 - Reads a `u32` discriminant.
 - If `0` (Unchecked): Calls `new_attr(src)` and wraps the result in `create::How::Unchecked`.
 - If `1` (Guarded): Calls `new_attr(src)` and wraps the result in `create::How::Guarded`.
 - If `2` (Exclusive): Calls `array::<{ NFS3_CREATEVERFSIZE }>(src)` to read the verifier, wraps it in `create::Verifier`, and wraps that in `create::How::Exclusive`.
 - For any other value, returns `Error::EnumDiscMismatch`.
3. **`new_attr` function**:
 - Uses `option(src, |s| u32(s))` to parse optional `mode`, `uid`, `gid`, and `size` fields.
 - Calls `set_time(src)` to parse `atime`.
 - Calls `set_time(src)` to parse `mtime`.
 - Constructs and returns `set_attr::NewAttr`.
4. **`set_time` function**:
 - Reads a `u32` discriminant.
 - If `0`: Returns `set_attr::SetTime::DontChange`.
 - If `1`: Returns `set_attr::SetTime::ToServer`.
 - If `2`: Calls `nfs_time(src)` and wraps the result in `set_attr::SetTime::ToClient`.
 - For any other value, returns `Error::EnumDiscMismatch`.
5. **`nfs_time` function**:
 - Reads a `u32` for seconds.
 - Reads a `u32` for nanoseconds.
 - Constructs and returns `file::Time`.

Edge Cases:
- **Invalid Discriminants**: If the `how` or `set_time` discriminants are not 0, 1, or 2, the parser returns `Error::EnumDiscMismatch`.
- **Stream Exhaustion**: If the stream ends prematurely during any read operation (e.g., reading the handle or verifier), the underlying `primitive` parsers will return an `Error::IO`.

Complexity:
- Time: O(1) for fixed-size fields (handles, integers). O(N) for variable-length fields (filenames), where N is the length of the string.
- Space: O(1) for stack-allocated structures. O(N) for heap-allocated strings within `file::Name`.

Determinism:
- Deterministic (Given the same input byte stream, the functions produce the same output or error).

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::parser::primitive`**:
 - **`option`**: This mechanism is critical for parsing `NewAttr`. In XDR, the fields of `sattr3` are optional, meaning each field is preceded by a boolean. The `option` function abstracts this logic, reading the boolean and parsing the value only if true.
 - **`array`**: Used to read the fixed 8-byte verifier in `Exclusive` mode. This ensures the exact byte count defined by `NFS3_CREATEVERFSIZE` is consumed.
 - **`u32` / `u64`**: Used to read all integer values, including discriminants and attribute values, handling Big-Endian conversion automatically.

- **From `nfs_mamont::parser::nfsv3::file`**:
 - **`handle` and `file_name`**: These functions encapsulate the parsing of the directory context. `handle` ensures the file handle is exactly 64 bytes, and `file_name` enforces length limits and UTF-8 validity, preventing invalid data from reaching the VFS layer.

- **From `nfs_mamont::vfs::create`**:
 - **`How` Enum**: The parsing logic in the `how` function is strictly driven by the variants of this enum (`Unchecked`, `Guarded`, `Exclusive`). The discriminant read from the wire determines which variant to construct and which subsequent parsing function (`new_attr` vs `array`) to invoke.

- **From `nfs_mamont::vfs::set_attr`**:
 - **`SetTime` Enum**: Similar to `How`, the `set_time` function maps a wire discriminant to the `DontChange`, `ToServer`, or `ToClient` variants, dictating whether additional time data needs to be parsed.

---

## 4. Data Model

Entities:
- This module does not define new public entities. It acts as a transformer, converting byte streams into the entities defined in `nfs_mamont::vfs::create` and `nfs_mamont::vfs::set_attr`.

Relations:
- **Transformation**: The functions in this module act as adapters between the `Read` stream and the VFS argument structs.
- **Composition**: `create::Args` contains `vfs::DirOpArgs` (parsed via `file` functions) and `create::How` (parsed via `how`). `create::How` contains `set_attr::NewAttr` (parsed via `new_attr`) or `Verifier`. `set_attr::NewAttr` contains `set_attr::SetTime` (parsed via `set_time`).

Global Invariants:
- **Discriminant Mapping**: The integer values 0, 1, and 2 are strictly mapped to specific enum variants in both `create::How` and `set_attr::SetTime` as per the NFSv3 specification.
- **Verifier Size**: In `Exclusive` mode, the verifier is always exactly `NFS3_CREATEVERFSIZE` bytes.

## 5. Error Model

Error Types:
- **`parser::Error`**: The primary error type used throughout the module.
 - `Error::EnumDiscMismatch`: Returned when the discriminant for `how` or `set_time` is not 0, 1, or 2.
 - `Error::IO`: Propagated from the underlying `Read` stream or from `primitive` parsers if the stream ends unexpectedly.

Error Propagation Strategy:
- **Propagation**: The `?` operator is used extensively to propagate errors from `primitive` parsers and `file` parsers.
- **Direct Return**: Logic errors (invalid discriminants) are returned immediately as `Error::EnumDiscMismatch`.

Recoverability:
- **Unrecoverable for the current operation**: If parsing fails, the stream cursor is likely in an undefined state relative to the higher-level RPC message. The caller (typically the RPC dispatcher) must discard the current request.

Panics:
- Allowed: No
- Conditions: The code uses `match` with catch-all `_` branches returning errors, and relies on `primitive` parsers which handle I/O errors via `Result`. No `unwrap` or `expect` calls are present.

---

## 6. Traits

List which external traits this module implements:
- None.

---

## 7. Overview

This module is used in order to **deserialize the arguments for the NFSv3 CREATE procedure** from the network wire format into the structured types required by the server's Virtual File System (VFS). The system contains a complex parser hierarchy where `primitive` handles raw bytes and `vfs::create` defines the semantic interface. This module sits in the middle, implementing the specific logic required to interpret the `CREATE3args` XDR structure.

A typical usage scenario of the system involves the RPC dispatcher receiving a `CREATE` request from an NFS client. The dispatcher identifies the procedure number and invokes the `args` function in this module. The function reads the directory handle and filename, then determines the creation mode (e.g., `Unchecked`, `Guarded`, or `Exclusive`) by reading a discriminant. Depending on the mode, it either parses a set of optional attributes or a verifier. The resulting `create::Args` struct is then passed to the VFS implementation to perform the actual file creation.

Inside the system, the following things happen and they use this module:
- **Conditional Parsing**: The `how` function implements the logic for the `createhow3` union. It distinguishes between creating a file with specific attributes (`Unchecked`/`Guarded`) versus creating a file exclusively using a verifier (`Exclusive`). This is critical for supporting the full range of NFSv3 file creation semantics.
- **Attribute Handling**: The `new_attr` function parses the `sattr3` structure, which allows the client to specify initial metadata for the file. It uses the `option` primitive to handle the fact that every field in `sattr3` is optional on the wire, allowing the client to update only specific attributes.
- **Timestamp Semantics**: The `set_time` function parses the `set_mode3` union, handling the three distinct ways NFSv3 allows timestamps to be set (don't change, set to server time, set to client time).

Without this module, the RPC layer would lack the specific logic to decode the `CREATE` procedure's arguments, which are more complex than simple reads or writes due to the union types and optional fields. This module ensures that the nuanced requirements of the NFSv3 protocol regarding file creation are correctly translated into the VFS layer's type-safe interface.