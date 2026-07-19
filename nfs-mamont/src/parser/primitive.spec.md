<!-- SPEC_HASH: 5e3709474d2d441c96f944929f224aee7e156b609892448469d13be3f110437f -->
# Module Specification

Module: nfs_mamont::parser::primitive
Rust File: src/parser/primitive.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`std::io::Read`**: The `Read` trait is the fundamental source of input for all parsing functions. It is used to read raw bytes from a stream (e.g., network socket or buffer).
- **`byteorder::{BigEndian, ReadBytesExt}`**: Used to read multi-byte integers (`u32`, `u64`, `i32`) in Big-Endian (network byte order), which is required by the XDR standard.
- **`num_traits::{FromPrimitive, ToPrimitive}`**: Used by the `variant` function to convert integer discriminants read from the stream into specific Rust enum variants.
- **`super::{Error, Result}`**: Uses the custom error type and result alias defined in the parent module (`parser::mod`). This allows the primitive parser to return specific errors (like `IO`, `EnumDiscMismatch`, `MaxElemLimit`) that are consistent with the rest of the application.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To provide a low-level, composable library of functions for parsing XDR (External Data Representation) encoded data from a byte stream.
- To enforce XDR alignment rules (4-byte boundaries) and encoding standards (Big-Endian integers, specific boolean representation).
- To abstract away the repetitive logic of reading lengths, padding, and performing type conversions, allowing higher-level protocol parsers to focus on structure.

Inputs:
- `src: &mut impl Read`: A mutable reference to a byte stream (e.g., TCP stream, file).
- `n: usize`: The size of the data just read or processed, used to calculate padding.
- `max_size: usize`: A limit for variable-length data to prevent denial-of-service via memory allocation.
- `cont: impl FnOnce(...)`: A closure used to parse the content of an optional type.

Outputs:
- Parsed primitive values (`u8`, `u32`, `bool`, `Vec<u8>`, `String`, etc.).
- `Result<T, Error>`: Indicates success with the parsed value or failure with a specific error.

Steps:
1. **Alignment Handling**: The `padding` function calculates the number of bytes required to reach the next 4-byte boundary using the formula `(ALIGNMENT - n % ALIGNMENT) % ALIGNMENT` and reads/discards that many bytes.
2. **Integer Parsing**: Functions like `u32`, `u64`, and `i32` use the `ReadBytesExt` trait to read the appropriate number of bytes in Big-Endian format.
3. **Boolean Parsing**: The `bool` function reads a `u32` and strictly validates that it is `0` (false) or `1` (true), returning `Error::EnumDiscMismatch` otherwise.
4. **Variable-Length Data Parsing**: The `vector` function reads a length prefix (`u32`), allocates a `Vec` of that size, reads the bytes into it, and then consumes the trailing padding.
5. **String Parsing**: The `string` function utilizes `vector` to read raw bytes and then attempts to convert them to a UTF-8 `String`, returning `Error::IncorrectString` if conversion fails.
6. **Optional Parsing**: The `option` function reads a boolean flag. If true, it invokes the provided closure to parse the value `T`; otherwise, it returns `None`.
7. **Enum Parsing**: The `variant` function reads a `u32` and uses `FromPrimitive::from_u32` to map it to an enum variant `T`.

Edge Cases:
- **Zero Padding**: If the data size `n` is already a multiple of 4, `padding` reads 0 bytes.
- **Invalid Booleans**: Any `u32` value other than 0 or 1 results in an error.
- **Size Limits**: `vec_max_size` checks the length prefix before allocation; if it exceeds `max_size`, it returns `Error::MaxElemLimit` immediately.
- **UTF-8 Validation**: `string` functions will fail if the byte sequence is not valid UTF-8.

Complexity:
- Time: O(N) for functions reading `N` bytes (e.g., `vector`, `array`). O(1) for fixed-size primitives.
- Space: O(N) for functions allocating buffers (`vector`, `string`). O(1) for primitives.

Determinism:
- Deterministic (Given the same input bytes, the functions produce the same output and errors).

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::rpc`**:
    - **`Error` Enum**: The module relies on the specific variants of the `Error` enum to report failures.
        - `Error::IO`: Wraps `std::io::Error` when `read_exact` fails.
        - `Error::EnumDiscMismatch`: Used when parsing booleans or enum variants with invalid discriminants.
        - `Error::IncorrectString`: Used when `String::from_utf8` fails.
        - `Error::ImpossibleTypeCast`: Used in `u32_as_usize` if the `u32` value cannot fit into a `usize` (though unlikely on 64-bit targets).
        - `Error::MaxElemLimit`: Used in `vec_max_size` when the length exceeds the allowed maximum.

- **From `byteorder`**:
    - **`ReadBytesExt`**: Provides the methods `read_u8`, `read_u32`, `read_u64`, and `read_i32` which handle the extraction of bytes from the stream and their assembly into integers.

- **From `num_traits`**:
    - **`FromPrimitive`**: Provides the `from_u32` static method used in the `variant` function to convert a raw integer read from the wire into a specific Rust enum type.

---

## 4. Data Model

Entities:
- **Primitive Types**: The module operates on standard Rust primitives: `u8`, `u32`, `u64`, `i32`, `bool`, `usize`.
- **Collections**: `Vec<u8>` (for opaque data), `[u8; N]` (for fixed arrays), `String` (for text).
- **Generic Types**: `T` (for the content of optional types and enum variants).

Relations:
- N/A (This module defines functions, not data structures with relationships).

Global Invariants:
- **Alignment**: All variable-length data structures (vectors, strings, arrays) are padded to a multiple of 4 bytes.
- **Endianness**: All multi-byte integers are encoded in Big-Endian (Network Byte Order).
- **Boolean Encoding**: Booleans are encoded as 32-bit integers where 0 is false and 1 is true.

---

## 5. Error Model

Error Types:
- **`nfs_mamont::rpc::Error`**: The sole error type returned by all functions in this module.

Error Propagation Strategy:
- **Custom Enum**: Functions return `Result<T, Error>`.
- **Mapping**: Standard I/O errors from `std::io::Read` are mapped to `Error::IO` using `map_err`. Logic errors (like invalid UTF-8 or size limits) are mapped to their specific `Error` variants.

Recoverability:
- **Stream Corruption**: If an `Error::IO` occurs, the underlying stream is likely in a bad state or closed; recovery usually requires closing the connection.
- **Protocol Violation**: `Error::EnumDiscMismatch` or `Error::IncorrectString` indicates the peer sent invalid data; the current message cannot be parsed, but the connection might remain valid for subsequent messages depending on the higher-level protocol logic.
- **Resource Exhaustion**: `Error::MaxElemLimit` prevents the server from allocating excessive memory, effectively protecting the process from OOM due to malicious length prefixes.

Panics:
- Allowed: No (The code uses `read_exact` and checked arithmetic; `vec![0u8; size]` is protected by `vec_max_size` in public paths, though `vector` could theoretically panic if the stream claims a size larger than available memory).

---

## 6. Traits

List which external traits this module implements:
- None (This module does not implement traits for its own types, as it defines no types. It uses traits from `std::io`, `byteorder`, and `num_traits`).

---

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here you MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to implement the fundamental decoding layer of the XDR (External Data Representation) standard used by ONC RPC and NFS protocols. It serves as the "byte-level interpreter" that translates a raw stream of bytes into the primitive data types (integers, booleans, strings, arrays) required to reconstruct complex protocol messages.

The system contains a set of highly optimized, stateless functions that strictly adhere to the XDR specification regarding alignment (4-byte boundaries) and integer encoding (Big-Endian). A typical usage scenario of the system involves a higher-level parser (e.g., for an NFS `READ` procedure) needing to read a file handle (an opaque byte array) and an offset (a 64-bit integer). The higher-level parser calls `array` or `vector` for the handle and `u64` for the offset. These functions handle the low-level bit manipulation, ensuring that the file handle is padded correctly and the offset is read in network byte order, abstracting this complexity away from the protocol logic.

Inside the system, the following things happen and they use this module:
- **Protocol Compliance**: The `padding` function ensures that the parser adheres to XDR alignment rules, which is critical for interoperability with standard NFS clients.
- **Safety**: The `vec_max_size` function enforces resource limits, preventing malicious or malformed packets from causing the server to allocate unbounded amounts of memory.
- **Type Mapping**: The `variant` function bridges the gap between the integer-based discriminants on the wire and the type-safe enums defined in the `rpc` module, ensuring that invalid protocol states are caught early and reported as `Error::EnumDiscMismatch`.

Without this module, every protocol parser in the `nfs_mamont` crate would need to manually implement alignment logic, endianness conversion, and safety checks, leading to code duplication and a high risk of implementation bugs. This module centralizes the "physics" of the wire format, allowing the rest of the codebase to focus on the "logic" of the protocol.