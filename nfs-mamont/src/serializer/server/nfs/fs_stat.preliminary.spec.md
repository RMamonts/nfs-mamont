<!-- SPEC_HASH: e492a57faef6dd68b7d71785af0030a2f75cb2a99c90962ee4dd0b803b5b541e -->
# Module Specification

Module: nfs_mamont::serializer::server::nfs::fs_stat
Rust File: src/serializer/server/nfs/fs_stat.rs

---

## 1. Dependencies

- **`std::io` / `std::io::Write`**: Provides the `Write` trait, which is the abstraction used for the output destination (e.g., a network buffer or stream). This allows the serializer to write bytes without knowing the specific concrete type of the buffer.
- **`crate::serializer::files::file_attr`**: A dependency used to serialize the `root_attr` field. Since `root_attr` is of type `Option<file::Attr>`, this module delegates the actual serialization of the file attributes to this specialized function.
- **`crate::serializer::{option, u32, u64}`**: Primitive serialization utilities. `option` handles the XDR encoding for optional fields (a boolean flag followed by the value if present). `u32` and `u64` handle the big-endian encoding of unsigned integers required by the XDR standard.
- **`crate::vfs::fs_stat`**: The source of the data models being serialized. It defines the `Success` and `Fail` structs that hold the file system statistics and attributes retrieved from the VFS layer.

---

## 2. Mechanics

### Mechanism 1: Serialization of Successful FSSTAT Response (`result_ok`)

**Intent:**
To convert a successful internal file system statistics result (`fs_stat::Success`) into the NFSv3 XDR wire format (`FSSTAT3resok`).

**Inputs:**
- `dest`: A mutable reference to a type implementing the `Write` trait, where the XDR bytes will be written.
- `arg`: A `fs_stat::Success` struct containing the file system attributes and statistics.

**Outputs:**
- `io::Result<()>`: Indicates success or an I/O error during the write operation.

**Steps:**
1. Serialize the `root_attr` field using the `option` helper. If the attribute is present, it invokes `file_attr` to write the attribute data; otherwise, it writes a boolean false indicating absence.
2. Serialize `total_bytes` as a 64-bit unsigned integer using `u64`.
3. Serialize `free_bytes` as a 64-bit unsigned integer using `u64`.
4. Serialize `available_bytes` as a 64-bit unsigned integer using `u64`.
5. Serialize `total_files` as a 64-bit unsigned integer using `u64`.
6. Serialize `free_files` as a 64-bit unsigned integer using `u64`.
7. Serialize `available_files` as a 64-bit unsigned integer using `u64`.
8. Serialize `invarsec` as a 32-bit unsigned integer using `u32`.

**Edge Cases:**
- If `arg.root_attr` is `None`, the `option` serializer ensures the correct XDR representation for an absent optional field is written.

**Complexity:**
- Time: O(1) — The function performs a fixed sequence of write operations regardless of the data values.
- Space: O(1) — No additional heap allocation is performed; data is written directly to the provided buffer.

**Determinism:**
- Deterministic — The same input struct will always produce the exact same sequence of bytes.

### Mechanism 2: Serialization of Failed FSSTAT Response (`result_fail`)

**Intent:**
To convert a failed internal file system statistics result (`fs_stat::Fail`) into the NFSv3 XDR wire format (`FSSTAT3resfail`). Note that in NFSv3, the failure body typically contains post-operation attributes (Weak Cache Consistency data) but not the error code itself (which is handled in the union discriminator).

**Inputs:**
- `dest`: A mutable reference to a type implementing the `Write` trait.
- `arg`: A `fs_stat::Fail` struct.

**Outputs:**
- `io::Result<()>`: Indicates success or an I/O error.

**Steps:**
1. Serialize the `root_attr` field using the `option` helper. If present, it invokes `file_attr` to write the attribute data.
2. The `error` field present in the `fs_stat::Fail` struct is **not** serialized by this function. This implies the error status is handled at a higher level (likely the union discriminator in the RPC response).

**Edge Cases:**
- If `arg.root_attr` is `None`, the `option` serializer handles the absence.

**Complexity:**
- Time: O(1).
- Space: O(1).

**Determinism:**
- Deterministic.

---

## 3. Dependency Mechanics

- **`crate::serializer::option`**: This mechanism is critical for handling fields that may or may not exist in the VFS response. It abstracts the XDR logic of writing a boolean flag followed by the data, ensuring the serializer doesn't need to manually check for `None` and write flags.
- **`crate::serializer::u32` / `crate::serializer::u64`**: These mechanisms handle the conversion of Rust integer types to the XDR standard (big-endian). They ensure that multi-byte integers are written in the correct network byte order.
- **`crate::serializer::files::file_attr`**: This mechanism encapsulates the complex logic of serializing file attributes (type, mode, size, timestamps, etc.). The current module relies on it to handle the `root_attr` field without needing to understand the internal structure of `file::Attr`.

---

## 4. Data Model

**Entities:**
- **`fs_stat::Success`**: A structure representing a successful file system status query. It contains optional root attributes and various counters for bytes and files (total, free, available), plus a volatility measure (`invarsec`).
- **`fs_stat::Fail`**: A structure representing a failed file system status query. It contains optional root attributes (post-operation attributes) and an error code (though the error code is not serialized by this specific module).

**Relations:**
- `fs_stat::Success` contains `Option<file::Attr>` (composition).
- `fs_stat::Fail` contains `Option<file::Attr>` (composition).

**Global Invariants:**
- The order of serialization in `result_ok` must strictly adhere to the NFSv3 `FSSTAT3resok` structure definition to ensure protocol compliance.
- The `invarsec` field must be serialized as a 32-bit integer, while the byte and file counts are 64-bit integers.

---

## 5. Error Model

**Error Types:**
- `std::io::Error`: Represents any I/O error that occurs while writing to the destination buffer (e.g., buffer full, broken pipe).

**Error Propagation Strategy:**
- Propagation via the `?` operator. If any underlying serialization call (`option`, `u64`, `file_attr`) returns an `Err`, the function immediately returns that error to the caller.

**Recoverability:**
- Not recoverable within this module. The module acts as a pure transformer; if writing fails, the operation is aborted, and the error is propagated up the stack to the RPC handler.

**Panics:**
- Allowed: No.
- Conditions: The code assumes that the provided `Write` implementation does not panic on write and that the helper functions (`option`, `u32`, etc.) are panic-free for valid inputs.

---

## 6. Traits

- **`std::io::Write`**: This module does not implement the trait but requires it as a bound on the `dest` parameter of its public functions.

---

## 7. Overview

This module is used in order to **translate high-level Virtual File System (VFS) objects representing file system statistics into the low-level XDR byte stream required by the NFSv3 network protocol**.

The system contains a **layered architecture where the VFS layer handles storage logic and domain-specific data structures (like `fs_stat::Success`), while the serializer layer handles protocol encoding**. This separation allows the internal logic to remain agnostic to network protocols and data representation formats.

A typical usage scenario of the system involves an NFS client sending a `FSSTAT` request. The server processes this request using the VFS, which returns a `Result<Success, Fail>`. The `result_ok` or `result_fail` function from this module is then called, depending on the result. It takes the Rust struct and writes the corresponding XDR bytes into the network response buffer.

Inside the system the following things happen and they use **primitive serializers (`u32`, `u64`, `option`) and composite serializers (`file_attr`) to construct the byte stream**. The module ensures that the specific fields defined in the NFSv3 specification—such as total bytes, free bytes, and file slots—are written in the correct order and format. Notably, for the failure case, the module serializes the `root_attr` (post-operation attributes) to support Weak Cache Consistency (WCC), allowing the client to update its cache even if the operation failed, while omitting the explicit error code as that is handled by the RPC status layer.