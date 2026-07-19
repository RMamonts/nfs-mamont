<!-- SPEC_HASH: 40a79b1cb4d8f2bfa0e39fc68e4a9d27419070a22e34e009d88160447006abf5 -->
# Module Specification

Module: nfs_mamont::parser::nlm
Rust File: src/parser/nlm/mod.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`crate::consts::nlm`**:
    - Used to access `LM_MAXSTRLEN` and `OPAQUE_HANDLE_SIZE`. These constants define the maximum allowed size for the caller name string and the opaque owner handle, respectively, which are enforced during parsing.
- **`crate::nlm::lock`**:
    - Used to access the `Nlm4Lock` struct. This struct is the target data structure for the `parse_lock` function, aggregating the parsed fields into a validated domain object.
- **`crate::nlm::OpaqueHandle`**:
    - Used as the return type for the `opaque_handle` function. It wraps the raw byte vector of the lock owner identifier into a type-safe structure.
- **`crate::parser::nfsv3::file`**:
    - Used to access the `file::handle` parsing function. NLMv4 uses NFSv3 file handles to identify files, so this dependency allows the NLM parser to reuse the standard file handle deserialization logic.
- **`crate::parser::primitive`**:
    - Used to access low-level XDR parsing functions: `i32`, `string_max_size`, `u64`, and `vector`. These are used to read the primitive data types (integers, strings, variable-length byte arrays) that make up the NLM arguments.
- **`crate::parser::{Error, Result}`**:
    - Used to standardize error handling. The module returns `Result<T>` where the error type is `parser::Error`, allowing consistent error propagation to the RPC layer.
- **`std::io::Read`**:
    - Used as the trait bound for the input source `src`. This allows the parser functions to accept any type that provides a byte stream (e.g., `TcpStream`, `Cursor`).

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

### Mechanism 1: Opaque Handle Decoding (`opaque_handle`)

**Intent:**
- To deserialize the lock-owner identifier (an opaque byte array) from the XDR-encoded stream and validate its size against the NLM protocol limits.

**Inputs:**
- `src: &mut impl Read`: A mutable reference to a byte stream containing the XDR-encoded opaque data.

**Outputs:**
- `Result<OpaqueHandle>`: The validated owner handle or an error if the data is malformed or exceeds the size limit.

**Steps:**
1. The function calls `primitive::vector(src)` to read a length-prefixed byte array from the stream. This handles the XDR encoding of variable-length opaque data (length + bytes + padding).
2. The resulting `Vec<u8>` is passed to `OpaqueHandle::new`.
3. If `OpaqueHandle::new` returns `Err` (indicating the vector length exceeds `OPAQUE_HANDLE_SIZE`), the error is mapped to `Error::BadFileHandle`.
4. If successful, the `OpaqueHandle` is returned.

**Edge Cases:**
- **Oversized Handle**: If the stream indicates a length greater than `OPAQUE_HANDLE_SIZE`, `OpaqueHandle::new` fails, and the function returns `Error::BadFileHandle`.
- **Insufficient Data**: If the stream ends prematurely, `primitive::vector` returns an `IO` error, which propagates.

**Complexity:**
- Time: O(N), where N is the length of the opaque handle in bytes.
- Space: O(N), for the allocated byte vector.

**Determinism:**
- Deterministic.

### Mechanism 2: Shared Lock Arguments Parsing (`parse_lock`)

**Intent:**
- To deserialize the common "lock arguments" block shared by NLMv4 procedures (LOCK, UNLOCK, TEST, CANCEL). This block contains the caller name, file handle, owner handle, system ID, and lock range.

**Inputs:**
- `src: &mut impl Read`: A mutable reference to a byte stream containing the XDR-encoded lock arguments.

**Outputs:**
- `Result<Nlm4Lock>`: The validated lock arguments structure or an error.

**Steps:**
1. **Caller Name**: The function calls `primitive::string_max_size(src, nlm::LM_MAXSTRLEN)` to read the client hostname, enforcing the maximum string length.
2. **File Handle**: The function calls `file::handle(src)` to read the NFSv3 file handle associated with the lock.
3. **Owner Handle**: The function calls `opaque_handle(src)` (defined in this module) to read the lock owner identifier.
4. **System ID**: The function calls `primitive::i32(src)` to read the system ID (typically a PID).
5. **Offset**: The function calls `primitive::u64(src)` to read the lock offset.
6. **Length**: The function calls `primitive::u64(src)` to read the lock length.
7. **Construction**: All parsed fields are passed to `Nlm4Lock::new`.
8. **Validation**: If `Nlm4Lock::new` returns `Err` (e.g., due to an empty caller name or other validation logic), the error is mapped to `Error::BadFileHandle`.

**Edge Cases:**
- **Invalid Caller Name**: If the caller name is empty or exceeds `LM_MAXSTRLEN`, `Nlm4Lock::new` fails, resulting in `Error::BadFileHandle`.
- **Invalid File Handle**: If the file handle parsing fails (e.g., wrong size), the error propagates.

**Complexity:**
- Time: O(N), where N is the total size of the variable-length fields (caller name, file handle, opaque handle).
- Space: O(N), for the allocated strings and vectors within `Nlm4Lock`.

**Determinism:**
- Deterministic.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::parser::primitive`**:
    - **`vector(src)`**: Reads a variable-length opaque byte array from the stream, handling the length prefix and XDR padding. Used by `opaque_handle`.
    - **`string_max_size(src, max)`**: Reads a string from the stream, ensuring it does not exceed `max` bytes. Used by `parse_lock` to read the caller name.
    - **`i32(src)` / `u64(src)`**: Reads signed/unsigned 32/64-bit integers in Big-Endian format. Used by `parse_lock` for system ID, offset, and length.

- **From `nfs_mamont::parser::nfsv3::file`**:
    - **`handle(src)`**: Parses an NFSv3 file handle. Used by `parse_lock` because NLM operations act on files identified by NFS handles.

- **From `nfs_mamont::nlm::lock`**:
    - **`Nlm4Lock::new(...)`**: A constructor that validates the lock arguments (specifically the caller name). Used by `parse_lock` to ensure the parsed data forms a valid lock request before returning it.

- **From `nfs_mamont::nlm`**:
    - **`OpaqueHandle::new(...)`**: A constructor that validates the size of the owner handle. Used by `opaque_handle` to enforce `OPAQUE_HANDLE_SIZE`.

---

## 4. Data Model

Entities:
- This module defines no public structs or enums. It acts as a factory for `Nlm4Lock` (defined in `crate::nlm::lock`) and `OpaqueHandle` (defined in `crate::nlm`).

Relations:
- **Construction**: The functions in this module construct instances of `Nlm4Lock` and `OpaqueHandle`.

Global Invariants:
- The input stream for `parse_lock` must strictly follow the NLMv4 XDR order: `caller_name` (string), `file_handle` (opaque), `opaque_handle` (opaque), `svid` (int32), `offset` (uint64), `length` (uint64).

## 5. Error Model

Error Types:
- **`crate::parser::Error`**: The canonical error type for the parser subsystem.

Error Propagation Strategy:
- **Mapping**: The module uses `map_err` to convert validation errors from `Nlm4Lock::new` and `OpaqueHandle::new` into `Error::BadFileHandle`. This treats structural validation failures (like oversized strings) similarly to invalid file handles at the RPC level.
- **Propagation**: Standard I/O errors from the `Read` trait or parsing errors from sub-parsers (`primitive`, `file`) are propagated using the `?` operator.

Recoverability:
- **Unrecoverable for the current message**: If parsing fails, the stream cursor is advanced to an undefined position relative to the message boundary. The RPC message cannot be processed further.

Panics:
- Allowed: No.
- Conditions: The code relies on fallible parsing operations and explicit error mapping; no `unwrap` or `expect` calls are present.

---

## 6. Traits

List which external traits this module implements:
- None.

---

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to **deserialize the common argument structures for the Network Lock Manager (NLM) version 4 protocol**. The system contains an NFS server that must support file locking. The NLM protocol defines several procedures (LOCK, UNLOCK, TEST, CANCEL) that share a significant portion of their argument structure (the "lock" block containing the file handle, owner, and range). This module centralizes the parsing logic for these shared structures, preventing code duplication across the individual procedure parsers.

A typical usage scenario of the system involves the RPC dispatcher receiving an NLM `LOCK` request. The dispatcher delegates the parsing to the `parser::nlm::lock` submodule. That submodule, in turn, calls the `parse_lock` function defined in this module to extract the common lock details. Similarly, if the request were `TEST` or `CANCEL`, their respective submodules would call the same `parse_lock` function. This ensures that the validation logic (e.g., checking caller name length) is consistent across all locking operations.

Inside the system, the following things happen and they use this module:
- **Protocol Interoperability**: By using `file::handle` from the NFSv3 parser, this module ensures that NLM correctly interprets the file identifiers generated by the NFS server, linking the locking subsystem directly to the file system namespace.
- **Validation Enforcement**: The module acts as a gatekeeper, converting raw bytes into `Nlm4Lock` and `OpaqueHandle` objects. It enforces size limits (`LM_MAXSTRLEN`, `OPAQUE_HANDLE_SIZE`) immediately upon parsing, rejecting malformed requests before they reach the core locking logic.
- **Code Reuse**: The `opaque_handle` and `parse_lock` functions are utilized by the procedure-specific submodules (`lock`, `unlock`, `test`, `cancel`), abstracting away the repetitive details of XDR field ordering and padding.

Without this module, the procedure-specific parsers would each have to implement the logic for reading the lock block, leading to potential inconsistencies (e.g., one parser checking the string limit while another forgets) and increased maintenance burden. This module provides the foundational "deserialization layer" for NLM arguments.