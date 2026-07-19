<!-- SPEC_HASH: 5e3709474d2d441c96f944929f224aee7e156b609892448469d13be3f110437f -->
# Module Specification

Module: nfs_mamont::parser::primitive
Rust File: /home/yarovoy/Documents/yadro/projects/oss/github/nfs-mamont/nfs-mamont/src/parser/primitive.rs

---

## 1. Dependencies

From the code analysis, the following external crates and standard library modules are used:

- **`std::io::Read`**: The core trait used to read bytes from a source (e.g., network stream or file). All parsing functions take a `&mut impl Read` to consume the byte stream.
- **`byteorder` (specifically `BigEndian` and `ReadBytesExt`)**: Used to read multi-byte integers (`u32`, `u64`, `i32`) ensuring they are interpreted in Big-Endian (network byte order), which is required by the XDR standard.
- **`num_traits` (specifically `FromPrimitive` and `ToPrimitive`)**: Used to convert raw integer discriminants read from the stream into specific Rust enum variants via the `variant` function.
- **`super` (parent module)**: Imports the `Error` enum and `Result` type alias.
  - *Assumption*: Based on the usage of `Error::IO`, `Error::EnumDiscMismatch`, `Error::IncorrectString`, `Error::MaxElemLimit`, and `Error::ImpossibleTypeCast`, and the fact that the `rpc` module defines these exact variants, it is assumed that the parent module (`parser::mod`) re-exports the `Error` type defined in `nfs_mamont::rpc`.

---

## 2. Mechanics

This module implements the low-level deserialization logic for the XDR (External Data Representation) standard, which is the data encoding format used by ONC RPC and NFS.

Intent:
- To provide a set of composable functions for reading primitive XDR data types from a byte stream.
- To enforce XDR encoding rules, specifically 4-byte alignment and Big-Endian byte order.
- To abstract away the manual bit-shifting and buffer management required for parsing binary protocols.

Inputs:
- `src`: A mutable reference to a type implementing the `Read` trait (the source of bytes).
- `n` / `max_size`: Integer constraints for sizes (e.g., padding length, maximum vector length).
- `cont`: A closure used to parse nested types (e.g., inside an `option`).

Outputs:
- Parsed Rust values (e.g., `u32`, `bool`, `Vec<u8>`, `String`).
- A `Result` type wrapping either the parsed value or an `Error`.

Steps:
1. **Alignment Handling**: Calculate the number of padding bytes required to reach the next 4-byte boundary using the formula `(ALIGNMENT - n % ALIGNMENT) % ALIGNMENT`. Read and discard these bytes.
2. **Integer Parsing**: Read bytes from the stream and interpret them as Big-Endian unsigned or signed integers.
3. **Boolean Parsing**: Read a `u32` and map `0` to `false` and `1` to `true`. Any other value results in an `EnumDiscMismatch` error.
4. **Complex Type Parsing**:
   - **Option**: Read a boolean flag. If true, invoke the provided closure to parse the contained value; otherwise, return `None`.
   - **Array/Vector**: Read the length (for vectors), read the exact number of data bytes into a buffer, and then read the trailing padding bytes.
   - **String**: Parse as a byte vector and validate the result as UTF-8.
   - **Enum Variant**: Read a `u32` discriminant and attempt to convert it to the target enum type using `FromPrimitive`.

Edge Cases:
- **Padding**: If the data length is already a multiple of 4, the padding length is 0.
- **Empty Vectors**: A length of 0 is valid; only padding is processed.
- **Invalid UTF-8**: The `string` functions will return an `IncorrectString` error if the bytes are not valid UTF-8.
- **Size Limits**: `vec_max_size` and `string_max_size` enforce a maximum allocation size to prevent potential denial-of-service attacks via memory exhaustion.

Complexity:
- Time: O(N) for variable-length types (vectors, strings, arrays), where N is the number of bytes. O(1) for fixed-size primitives.
- Space: O(N) for variable-length types (allocates a `Vec` or `String`). O(1) for primitives.

Determinism:
- Deterministic. Given the same input byte stream, the functions will always produce the same output or error.

---

## 3. Dependency Mechanics

From the specifications of the dependencies, the following mechanisms are relevant:

- **`nfs_mamont::rpc::Error`**: This module relies on the specific error variants defined in the RPC module to report parsing failures.
  - `Error::IO`: Wraps `std::io::Error` when the underlying stream fails.
  - `Error::EnumDiscMismatch`: Used when a boolean is not 0/1 or when an enum discriminant cannot be mapped.
  - `Error::IncorrectString`: Used when UTF-8 validation fails.
  - `Error::MaxElemLimit`: Used when a vector/string exceeds the specified `max_size`.
  - `Error::ImpossibleTypeCast`: Used when converting a `u32` to `usize` fails (e.g., on platforms where `usize` is smaller than `u32`).

---

## 4. Data Model

Entities:
- **`ALIGNMENT`**: A constant (`usize`) equal to 4, representing the XDR alignment requirement.

Relations:
- N/A (This module contains only functions and constants).

Global Invariants:
- All multi-byte integers are encoded in Big-Endian format.
- All data types are aligned to 4-byte boundaries.
- Booleans are encoded as 32-bit integers (0 for false, 1 for true).

---

## 5. Error Model

Error Types:
- **`Error`**: The error enum imported from the parent module (originating from `nfs_mamont::rpc`).

Error Propagation Strategy:
- Custom enum (`Error`). The functions use the `?` operator to propagate errors from underlying `read` calls (mapped to `Error::IO`) or validation logic (mapped to specific `Error` variants).

Recoverability:
- Generally unrecoverable for the current parsing operation. If a primitive parse fails (e.g., invalid alignment or IO error), the stream state is often compromised or the data is corrupt, requiring the connection or operation to be reset.

Panics:
- Allowed: No. The code uses `read_exact` and `map_err` to handle I/O errors gracefully. Buffer sizes are fixed or checked against limits before allocation.

---

## 6. Traits

The module does not implement any traits for public types. It uses the following external traits as bounds:
- **`std::io::Read`**: Bound on the `src` parameter for all parsing functions.
- **`num_traits::FromPrimitive`**: Bound on the type parameter `T` in the `variant` function.

---

## 7. Overview

This module is used in order to implement the physical layer of the ONC RPC protocol parsing, specifically handling the XDR (External Data Representation) standard. It translates the raw, untyped byte stream received from the network into strongly-typed Rust primitives while strictly enforcing the encoding rules defined in RFC 1014 (XDR).

This system contains a collection of stateless parsing utilities that act as the "alphabet" for the higher-level NFS parser. While the `rpc` module defines the *vocabulary* (the structs and enums representing the protocol), this `primitive` module defines the *mechanics of reading* those definitions from a wire format.

A typical usage scenario of the system involves a higher-level parser (e.g., parsing an NFS `LOOKUP` request) needing to read a file path (a string). It calls `string_max_size` from this module, passing the network stream and a maximum path length. The `primitive` module reads a 32-bit length, checks it against the limit, reads that many bytes, reads the subsequent padding bytes to align to the next 4-byte boundary, and validates the UTF-8 encoding. If successful, it returns the `String` to the caller.

Inside the system, the following things happen and they use:
- **Endian Conversion**: Uses `byteorder::BigEndian` to ensure integers are read in network byte order.
- **Memory Safety**: Uses `vec_max_size` to prevent untrusted network data from causing excessive memory allocation.
- **Protocol Compliance**: Uses `padding` to ensure the read cursor is always positioned correctly for the next XDR field, as XDR requires all data to be aligned on 4-byte boundaries.