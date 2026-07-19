<!-- SPEC_HASH: 64de296f5cc42a2f3ab005506d5a789759e937af0180ce84a329d8e2237809c7 -->
# Module Specification

Module: nfs_mamont::serializer
Rust File: src/serializer/mod.rs

---

## 1. Dependencies

From the code analysis, the following external crates and modules are used:

- **`std::io`**: Used to provide the `Write` trait, which serves as the abstraction for the destination byte sink (e.g., network socket or buffer), and `Error`/`ErrorKind` for handling I/O failures and validation errors (e.g., `InvalidInput` for size overflows).
- **`byteorder`**: Used specifically for the `WriteBytesExt` trait, which provides methods like `write_u32::<BigEndian>` to handle the conversion of Rust integers to big-endian bytes required by the XDR standard.
- **`num_traits`**: Used for the `ToPrimitive` trait, which enables the conversion of enum discriminants (or other types) to primitive integers (`u32`) for serialization.
- **`crate::serializer::files`**: (Internal submodule) Although technically a child module in the source tree, the provided specification indicates it consumes the primitives defined here to map VFS types to XDR formats.
- **`crate::serializer::server`**: (Internal submodule) Acts as a parent for protocol-specific serializers (NFS, MOUNT, NLM) which likely utilize these primitives for constructing RPC messages.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To provide a foundational set of stateless functions that implement the XDR (External Data Representation) standard encoding rules.
- To abstract away the low-level details of byte order (Big Endian), alignment (4-byte boundaries), and padding, allowing higher-level modules to focus on protocol structure.

Inputs:
- `dest`: A mutable reference to a type implementing the `std::io::Write` trait, where the serialized bytes will be written.
- Data values: Rust primitives (`u32`, `u64`, `bool`, `usize`), slices (`&[u8]`), strings (`&str`), options (`Option<T>`), and enums (via `ToPrimitive`).

Outputs:
- `io::Result<()>`: Indicates the success of the write operation or contains an `io::Error` if writing failed or validation constraints were violated.

Steps:
1. **Alignment Calculation**: The `padding` function determines the number of zero bytes required to align the current write position to the next 4-byte boundary (XDR `ALIGNMENT`) and writes them.
2. **Integer Serialization**: Functions `u32` and `u64` convert the input integer into a big-endian byte array and write it to the destination.
3. **Boolean Serialization**: The `bool` function maps `true` to `1u32` and `false` to `0u32` and serializes it as a 32-bit integer.
4. **Variable-Length Data Serialization**: The `vector` function writes the length of the data as a 32-bit unsigned integer, followed by the raw bytes, followed by alignment padding.
5. **Fixed-Length Data Serialization**: The `array` function writes the raw bytes of a fixed-size array followed by alignment padding.
6. **Optional Data Serialization**: The `option` function writes a boolean discriminator. If `Some(val)`, it writes `true` and executes a closure to serialize the inner value; if `None`, it writes `false`.
7. **Bounded Serialization**: Functions like `vec_max_size` and `string_max_size` validate that the input length does not exceed a specified maximum before delegating to the standard serialization logic.
8. **Enum Discriminant Serialization**: The `variant` function converts a type implementing `ToPrimitive` (typically an enum) to a `u32` and serializes it.

Edge Cases:
- **Length Overflow**: `vector` returns an `InvalidInput` error if the slice length exceeds `u32::MAX`.
- **Size Constraints**: `vec_max_size` and `string_max_size` return an `InvalidInput` error if the input length exceeds the provided `max_size`.
- **Conversion Failure**: `variant` and `usize_as_u32` return an `InvalidInput` error if the value cannot be converted to `u32` (e.g., a `usize` larger than 32 bits on a 64-bit platform).

Complexity:
- Time: O(N), where N is the number of bytes written to the destination.
- Space: O(1) auxiliary space (excluding the buffer managed by the `Write` implementation).

Determinism:
- Deterministic. Given the same input values and `Write` implementation, the output byte sequence is identical.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::serializer::files`**:
 - **Primitive Consumption**: The `files` module relies on the primitives defined in this module (specifically `u32`, `u64`, `array`, `option`, `string_max_size`, `variant`, and `usize_as_u32`) to serialize complex VFS types like `file::Attr` and `file::Handle`. This confirms that the primitives provided here must correctly handle alignment and padding to satisfy NFSv3 protocol requirements.
 - **Error Propagation**: The `files` module expects these primitives to return `std::io::Result`, propagating any I/O or validation errors up the stack.

- **From `nfs_mamont::serializer::server`**:
 - **Write Abstraction**: The `server` module (and its submodules) utilizes the `std::io::Write` trait as the common destination for serialization. This module's functions accept `&mut impl Write`, making them compatible with the buffering and network transmission strategies employed by the `server` module.

---

## 4. Data Model

Entities:
- **XDR Primitives**: Stateless functions representing the basic data types of the XDR language (Integer, Unsigned Integer, Boolean, Opaque Data, String, Optional, Enum).
- **Byte Stream**: The implicit sequence of bytes written to the `dest` buffer, conforming to Big Endian order and 4-byte alignment.

Relations:
- **Composition**: Higher-level data structures (defined in dependent modules like `files`) are composed of these primitives. For example, an NFS file attribute structure is composed of integers, opaque arrays, and optional fields, all serialized using these primitives.

Global Invariants:
- **Alignment**: Every serialized field is implicitly padded to a 4-byte boundary.
- **Endianness**: All multi-byte integers are encoded in Big Endian (network byte order).

## 5. Error Model

Error Types:
- **`std::io::Error`**: The sole error type returned by all public functions.

Error Propagation Strategy:
- **Direct Propagation**: Errors from the underlying `Write` implementation (e.g., broken pipe) are returned immediately.
- **Validation Errors**: The module explicitly creates `io::Error` with `ErrorKind::InvalidInput` for logical errors such as buffer overflows, size limit violations, or failed integer conversions.

Recoverability:
- **Recoverable**: All functions return `Result`, allowing the caller to handle serialization failures gracefully (e.g., by aborting the RPC request).

Panics:
- **Allowed**: No.
- **Conditions**: The code avoids panics by using checked conversions (e.g., `try_into()`, `ok_or()`) and returning errors for all failure modes.

---

## 6. Traits

List which external traits this module implements:
- None.

---

## 7. Overview

This module is used in order to **implement the XDR (External Data Representation) standard encoding**, which is the mandatory data serialization format for ONC RPC and the protocols built on top of it, such as NFS and NLM. The system requires a strict binary format to ensure interoperability between heterogeneous clients and servers. This module serves as the foundational "vocabulary" for constructing these binary messages.

The system contains a **layered serialization architecture**. At the bottom layer, this module defines the atomic operations for writing integers, booleans, and byte arrays with strict adherence to alignment and endianness rules. Above it, modules like `serializer::files` and `serializer::server` (and its submodules) use these atomic operations to compose complex domain-specific structures (like file attributes, directory entries, and RPC headers) without having to repeatedly manage low-level bit manipulation.

A typical usage scenario of the system involves the server preparing an NFS reply. The server logic holds a Rust structure representing the result (e.g., file attributes). The `serializer::files` module breaks this structure down into fields (type, mode, size, etc.) and calls the appropriate functions from this module (e.g., `u32` for the mode, `u64` for the size, `array` for the file handle). These functions write the raw bytes into a buffer managed by the `server` module, which is then transmitted to the client.

Inside the system, the following things happen and they use this module:
1.  **Protocol Compliance**: The `server` module ensures that the RPC message framing is correct, while this module ensures that the payload data within that frame conforms to XDR.
2.  **Data Transformation**: The `files` module transforms high-level VFS (Virtual File System) concepts into a stream of bytes by invoking these primitives.
3.  **Error Handling**: If a data structure is too large to fit in a 32-bit length field (a constraint of XDR), this module detects the overflow and returns an error, preventing the generation of a malformed network packet.

Without this module, the higher-level serialization logic would be cluttered with repetitive code for byte swapping, padding calculation, and boundary checking, increasing the risk of bugs and non-compliance with the NFS protocol standards.