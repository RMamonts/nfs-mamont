<!-- SPEC_HASH: 64de296f5cc42a2f3ab005506d5a789759e937af0180ce84a329d8e2237809c7 -->
# Module Specification

Module: nfs_mamont::serializer
Rust File: src/serializer/mod.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **std::io**: Used to provide the `Write` trait, which defines the interface for the destination byte stream (e.g., a network buffer or file), and `io::Error`/`ErrorKind` for reporting I/O failures or invalid input constraints (such as integer overflow).
- **byteorder**: Used to handle the conversion of Rust integers to the specific Big Endian byte order required by the XDR standard via the `WriteBytesExt` trait.
- **num_traits**: Used for the `ToPrimitive` trait, which enables generic conversion of enum discriminants or custom numeric types to `u32` for serialization.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To provide a foundational set of stateless functions that serialize Rust primitive and composite types into the XDR (External Data Representation) binary format.
- To enforce strict adherence to the XDR standard, specifically Big Endian byte order and 4-byte alignment, ensuring data interoperability across different architectures.

Inputs:
- `dest`: A mutable reference to a type implementing the `std::io::Write` trait, acting as the byte sink.
- Data values: Primitive types (`u32`, `u64`, `bool`), slices (`&[u8]`), strings (`&str`), or generic containers (`Option<T>`) to be serialized.

Outputs:
- `io::Result<()>`: Indicates successful writing of the serialized bytes to the destination or an error if the write operation fails or input constraints are violated.

Steps:
1. **Alignment Calculation**: The `padding` function calculates the number of zero bytes required to reach the next 4-byte boundary using the formula `(ALIGNMENT - n % ALIGNMENT) % ALIGNMENT` and writes them to the destination.
2. **Integer Serialization**: The `u32` and `u64` functions utilize the `byteorder` crate to write integers in Big Endian format. The `usize_as_u32` function performs a checked conversion, returning an error if the value exceeds `u32::MAX`.
3. **Boolean Serialization**: The `bool` function encodes `true` as `1u32` and `false` as `0u32`.
4. **Variable-Length Data Serialization**: The `vector` function writes the length of the data as a 32-bit unsigned integer, followed by the data bytes, followed by alignment padding. The `vec_max_size` variant validates the length against a `max_size` limit before proceeding.
5. **String Serialization**: The `string_max_size` function treats strings as UTF-8 byte arrays and delegates to `vec_max_size` to enforce length bounds and proper encoding.
6. **Optional Data Serialization**: The `option` function writes a boolean discriminator (indicating presence). If `true`, it invokes a provided closure `cont` to serialize the encapsulated value.
7. **Variant Serialization**: The `variant` function converts a generic type `T` implementing `ToPrimitive` into a `u32` and writes it as an enum discriminant.

Edge Cases:
- **Integer Overflow**: `usize_as_u32` returns `ErrorKind::InvalidInput` if the `usize` value cannot fit into a `u32`.
- **Length Limits**: `vector` returns `ErrorKind::InvalidInput` if the slice length exceeds `u32::MAX`. `vec_max_size` and `string_max_size` return `ErrorKind::InvalidInput` if the length exceeds the specified `max_size`.
- **Conversion Failure**: `variant` returns `ErrorKind::InvalidInput` if `ToPrimitive::to_u32` returns `None`.

Complexity:
- Time: O(N), where N is the number of bytes written to the destination.
- Space: O(1) auxiliary space (excluding the buffer managed by the `Write` implementation).

Determinism:
- Deterministic

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::serializer::files`**: This submodule acts as the primary consumer of the current module's primitives. It utilizes `u32`, `u64`, `array`, `option`, `string_max_size`, `variant`, and `usize_as_u32` to serialize complex VFS (Virtual File System) structures like file attributes, handles, and directory arguments into the NFSv3 wire format. The dependency relies on the current module to handle the low-level details of XDR encoding (endianness and padding) so that it can focus on mapping domain-specific types to protocol structures.

---

## 4. Data Model

Entities:
- **XDR Primitives**: Stateless functions representing the basic building blocks of the XDR format (integers, booleans, opaque data, variable-length arrays).
- **Alignment Constraint**: A constant `ALIGNMENT` set to 4 bytes, governing the padding logic for all serialized data.

Relations:
- **Composition**: Higher-level serializers (e.g., in `serializer::files`) compose these primitives to construct complex protocol messages.
- **Constraint Enforcement**: The `vector` and `string` functions implicitly depend on the `padding` function to satisfy XDR alignment requirements after writing variable-length data.

Global Invariants:
- All multi-byte integers are written in Big Endian (network) byte order.
- Every serialized field is padded to a 4-byte boundary, ensuring the total length of any serialized XDR data structure is a multiple of 4.

## 5. Error Model

Error Types:
- `std::io::Error`

Error Propagation Strategy:
- Errors are propagated using the `?` operator. This includes I/O errors from the underlying `Write` implementation and logical errors (e.g., `InvalidInput`) for data that violates XDR constraints (like exceeding maximum length).

Recoverability:
- Recoverable. The functions return `Result`, allowing the caller to handle serialization failures gracefully (e.g., by aborting the RPC request).

Panics:
- Allowed: No
- Conditions: The code avoids panics by using checked conversions (e.g., `try_into`, `ok_or`) and returning `io::Result` for all failure modes.

---

## 6. Traits

List which external traits this module implements:
- None. This module defines free-standing serialization functions and does not implement traits for its own types.

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here you MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to establish the binary communication protocol layer for an NFSv3 server implementation. The system requires a mechanism to translate high-level, architecture-independent Rust data structures into a standardized wire format (XDR) that can be interpreted by heterogeneous NFS clients. The XDR standard mandates specific byte ordering (Big Endian) and data alignment (4-byte boundaries) that are not native to all CPU architectures or Rust's default memory layout.

A typical usage scenario of the system involves the server preparing a response to a file system request, such as `READ` or `GETATTR`. The VFS layer generates a response object containing file attributes or data chunks. The system then invokes the serialization functions in `serializer::files`, which in turn delegate to the primitives in this module. For example, serializing a file attribute requires writing a 32-bit mode, a 64-bit file size, and timestamps. This module provides the `u32`, `u64`, and `variant` functions to ensure these integers are correctly byte-swapped and written to the network buffer. Without this module, the system would lack the ability to generate compliant NFSv3 packets, rendering the server unable to communicate with standard clients. It serves as the critical translation layer between the server's internal logic and the external network protocol.