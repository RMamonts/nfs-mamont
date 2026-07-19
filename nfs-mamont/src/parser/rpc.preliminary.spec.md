<!-- SPEC_HASH: 44efd1ff1dd78b25163474c4b97b37c92232742b38223f91b81ab193b4c56628 -->
# Module Specification

Module: nfs_mamont::parser::rpc
Rust File: src/parser/rpc.rs

---

## 1. Dependencies

From the code analysis, the following external crates and standard library modules are used:

- **`std::io::Read`**: The core trait used to read bytes from a source. It is used as a bound for the `src` parameter in parsing functions to consume the byte stream.
- **`crate::parser::primitive`**: Used to access low-level XDR parsing utilities. Specifically, `variant` is used to deserialize enum discriminants into `AuthFlavor`, and `vec_max_size` is used to deserialize variable-length byte arrays with a safety limit.
- **`crate::rpc`**: Used to import protocol-specific type definitions and constants. `AuthFlavor` and `OpaqueAuth` define the structure of the data being parsed, and `MAX_AUTH_SIZE` provides the security constraint for the authentication body length.
- **`crate::parser::Result`**: Used as the return type for parsing functions, aliasing the `Result` type that wraps the module's specific `Error` enum.

---

## 2. Mechanics

This module implements the parsing logic for specific ONC RPC protocol structures, bridging the gap between generic XDR primitives and high-level RPC types.

Intent:
- To provide a parser for the `OpaqueAuth` structure, which encapsulates authentication credentials in RPC messages.
- To define a data structure (`RpcMessage`) that represents the core identifying fields of an RPC call (program, procedure, version, and authentication).

Inputs:
- `src`: A mutable reference to a type implementing the `Read` trait (the source of bytes).

Outputs:
- `OpaqueAuth`: A struct containing the parsed authentication flavor and the raw bytes of the authentication body.
- `RpcMessage`: A public struct definition intended to hold parsed RPC message metadata.

Steps:
1. **Authentication Parsing (`auth` function)**:
   - Invoke `variant::<AuthFlavor>(src)` to read a 32-bit integer discriminant from the stream and map it to the `AuthFlavor` enum.
   - Invoke `vec_max_size(src, MAX_AUTH_SIZE)` to read the length-prefixed byte vector representing the authentication body. This step ensures the vector does not exceed the protocol-defined maximum size (400 bytes).
   - Construct and return an `OpaqueAuth` struct containing the parsed flavor and body.

Edge Cases:
- **Invalid Discriminant**: If the integer read for the authentication flavor does not correspond to a valid `AuthFlavor` variant, the underlying `variant` function will return an `Error::EnumDiscMismatch`.
- **Size Limit Exceeded**: If the length prefix of the authentication body indicates a size greater than `MAX_AUTH_SIZE`, the `vec_max_size` function will return an `Error::MaxElemLimit`.
- **Insufficient Data**: If the stream ends before the required fields are read, an `Error::IO` will be propagated.

Complexity:
- Time: O(N), where N is the size of the authentication body in bytes.
- Space: O(N), where N is the size of the authentication body (allocated into a `Vec`).

Determinism:
- Deterministic. Given the same input byte stream, the function will always produce the same `OpaqueAuth` structure or the same error.

---

## 3. Dependency Mechanics

From the specifications of the dependencies, the following mechanisms are relevant:

- **`nfs_mamont::parser::primitive::variant`**: This mechanism is critical for reading the `AuthFlavor`. It reads a 32-bit integer and uses `FromPrimitive` to convert it to the enum. It handles the mapping of wire-format integers to semantic Rust types.
- **`nfs_mamont::parser::primitive::vec_max_size`**: This mechanism is critical for reading the authentication body. It enforces a memory allocation limit (`MAX_AUTH_SIZE`) to prevent denial-of-service attacks via large payloads. It also handles the XDR padding requirements automatically after reading the vector data.
- **`nfs_mamont::rpc::OpaqueAuth`**: This is the target data structure. The `auth` function acts as a constructor for this type by populating its `flavor` and `body` fields from the stream.
- **`nfs_mamont::rpc::Error`**: The error types propagated by this module (e.g., `IO`, `EnumDiscMismatch`, `MaxElemLimit`) are defined here, providing a unified error handling strategy for the parser.

---

## 4. Data Model

Entities:
- **`RpcMessage`**: A public structure representing the metadata of an RPC call.
  - `program`: `u32` - The remote program number being called.
  - `procedure`: `u32` - The specific procedure number within the program.
  - `version`: `u32` - The version number of the protocol.
  - `cred`: `OpaqueAuth` - The authentication credentials of the caller.
  - `verf`: `OpaqueAuth` - The verification verifier (often used in responses).

Relations:
- `RpcMessage` → `OpaqueAuth` (Composition: `cred` and `verf` fields).
- `OpaqueAuth` → `AuthFlavor` (Composition: `flavor` field).

Global Invariants:
- The `body` field of `OpaqueAuth` parsed by the `auth` function will never exceed `MAX_AUTH_SIZE` (400 bytes).

---

## 5. Error Model

Error Types:
- **`Error`**: The error enum defined in `nfs_mamont::rpc` and re-exported via `crate::parser::Result`.

Error Propagation Strategy:
- Custom enum (`Error`). The module uses the `?` operator to propagate errors returned by the `primitive` module functions.

Recoverability:
- Generally unrecoverable for the current message parsing. If authentication data cannot be parsed (e.g., invalid flavor or size limit exceeded), the RPC message is considered malformed and cannot be processed safely.

Panics:
- Allowed: No. The module relies on the underlying `primitive` module which handles I/O errors and allocation limits gracefully.

---

## 6. Traits

The module implements the following external traits (via `derive` macros):
- **`Debug`**: Implemented on `RpcMessage`.

The module uses the following external traits as bounds:
- **`std::io::Read`**: Bound on the `src` parameter in the `auth` function.

---

## 7. Overview

This module is used in order to implement the parsing logic for the ONC RPC protocol's authentication and message targeting layers. It translates the raw byte stream into structured Rust types that represent the specific semantics of an RPC call, such as which program is being invoked and what credentials are being presented.

This system contains a specialized parser that sits atop the generic XDR parsing primitives. While the `primitive` module handles the mechanics of reading integers and aligned byte arrays, this module applies those mechanics to the specific data structures defined in the `rpc` module. It enforces protocol-specific constraints, such as the maximum size of authentication data, to ensure the security and stability of the server.

A typical usage scenario of the system involves a network handler receiving an RPC Call message. After reading the generic RPC header (XID and version), the handler needs to parse the credentials to authorize the request. It calls the `auth` function, passing the network stream. The `auth` function reads the authentication flavor (e.g., `AUTH_NONE` or `AUTH_SYS`) and the opaque body bytes, ensuring the body size is within the 400-byte limit defined by the RPC specification. The resulting `OpaqueAuth` struct is then placed into an `RpcMessage` struct, which is populated with the program, procedure, and version numbers read elsewhere in the parsing process.

Inside the system, the following things happen and they use:
- **Type Safety**: Uses `variant` to convert raw integers into the `AuthFlavor` enum, ensuring that only valid authentication types are represented in the system.
- **Resource Management**: Uses `vec_max_size` to strictly limit memory allocation when reading the variable-length authentication body, preventing malicious clients from exhausting server memory.
- **Protocol Encapsulation**: Uses `RpcMessage` to aggregate the parsed fields, providing a clean interface for the higher-level logic that determines which service handler to invoke.