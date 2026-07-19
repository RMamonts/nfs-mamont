<!-- SPEC_HASH: 44efd1ff1dd78b25163474c4b97b37c92232742b38223f91b81ab193b4c56628 -->
# Module Specification

Module: nfs_mamont::parser::rpc
Rust File: src/parser/rpc.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`std::io::Read`**: The `Read` trait is used as the source abstraction for the input byte stream. It allows the parsing functions to operate on any type that provides bytes (e.g., `TcpStream`, `&[u8]`).
- **`crate::parser::primitive::{variant, vec_max_size}`**: These functions are used to perform the actual XDR decoding. `variant` parses the authentication flavor enum, and `vec_max_size` parses the opaque authentication body while enforcing a size limit.
- **`crate::parser::Result`**: A type alias for the result type used throughout the parser subsystem, ensuring consistent error handling.
- **`crate::rpc::{AuthFlavor, OpaqueAuth, MAX_AUTH_SIZE}`**: These types and constant define the structure of the data being parsed. `AuthFlavor` is the target enum, `OpaqueAuth` is the target struct, and `MAX_AUTH_SIZE` provides the security constraint for the authentication data length.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To provide a parser for the `OpaqueAuth` structure, which encapsulates the authentication credentials and verifier in an ONC RPC message.
- To define the `RpcMessage` structure, which serves as a container for the core identifying fields of an RPC call (program, version, procedure) and its authentication context.

Inputs:
- `src: &mut impl Read`: A mutable reference to a byte stream representing the incoming RPC message data.

Outputs:
- `Result<OpaqueAuth>`: The parsed authentication structure containing the flavor and the opaque body bytes.
- `RpcMessage`: A public struct definition (no parser provided in this module) holding the program number, procedure number, version number, and two `OpaqueAuth` instances (`cred` and `verf`).

Steps:
1. **Authentication Parsing (`auth` function)**:
   - The function calls `variant::<AuthFlavor>(src)` to read a 32-bit integer discriminant from the stream and map it to the `AuthFlavor` enum.
   - It calls `vec_max_size(src, MAX_AUTH_SIZE)` to read a length-prefixed byte vector. This function ensures the vector length does not exceed `MAX_AUTH_SIZE` (400 bytes), preventing potential denial-of-service attacks via large allocations.
   - It combines the flavor and body into an `OpaqueAuth` struct and returns it.

Edge Cases:
- **Size Limit**: If the length prefix of the authentication body exceeds `MAX_AUTH_SIZE`, `vec_max_size` returns an error, causing `auth` to fail immediately.
- **Invalid Flavor**: If the discriminant read by `variant` does not correspond to a valid `AuthFlavor` variant, an error is returned.

Complexity:
- Time: O(N) where N is the size of the authentication body (due to reading bytes).
- Space: O(N) for the allocation of the authentication body `Vec<u8>`.

Determinism:
- Deterministic (Given the same input bytes, the function produces the same output or error).

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::parser::primitive`**:
 - **`variant`**: Used to parse the `AuthFlavor` enum. It reads a `u32` and uses `FromPrimitive` to convert it to the specific enum variant, handling potential discriminant mismatches.
 - **`vec_max_size`**: Used to parse the body of the `OpaqueAuth`. It reads a length prefix, allocates a `Vec`, and enforces the `MAX_AUTH_SIZE` limit to ensure memory safety.

- **From `nfs_mamont::rpc`**:
 - **`OpaqueAuth`**: The target data structure being constructed by the `auth` function.
 - **`AuthFlavor`**: The enum type being parsed by the `variant` function.
 - **`MAX_AUTH_SIZE`**: The constant value (400) passed to `vec_max_size` to bound the memory allocation for the authentication data.

---

## 4. Data Model

Entities:
- **`RpcMessage`**: A public structure representing the identity and security context of an RPC call.
 - `program: u32`: The RPC program number being called.
 - `procedure: u32`: The specific procedure number within the program.
 - `version: u32`: The version number of the protocol.
 - `cred: OpaqueAuth`: The authentication credentials of the caller.
 - `verf: OpaqueAuth`: The verifier (often used in responses or subsequent steps, though parsed here).
- **`OpaqueAuth`**: (Defined in dependency, used here) A structure holding an `AuthFlavor` and a `Vec<u8>` body.

Relations:
- **Composition**: `RpcMessage` contains two instances of `OpaqueAuth` (`cred` and `verf`).

Global Invariants:
- **Auth Size Limit**: The `body` field of any `OpaqueAuth` parsed via the `auth` function is guaranteed to be at most `MAX_AUTH_SIZE` bytes.

## 5. Error Model

Error Types:
- **`crate::rpc::Error`**: The error type returned via the `Result` alias.

Error Propagation Strategy:
- **Direct Propagation**: The `auth` function uses the `?` operator to propagate errors returned by `variant` and `vec_max_size`. This includes I/O errors, enum discriminant mismatches, and size limit violations.

Recoverability:
- **Stream Corruption**: If an error occurs, the stream position is undefined relative to the message boundary. The caller typically cannot recover the current message and must discard or close the connection.

Panics:
- Allowed: No (The code relies on checked operations in dependencies).

---

## 6. Traits

List which external traits this module implements:
- None.

---

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here you MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to **parse the security and routing metadata of an ONC RPC Call message**. It provides the specific logic required to interpret the authentication fields (`cred` and `verf`) and defines the structure that holds the program identifiers necessary to dispatch the request to the correct service handler.

The system contains a specialized parser for `OpaqueAuth` that combines generic XDR primitives (from `parser::primitive`) with RPC-specific constraints (from `rpc`). A typical usage scenario of the system involves the main RPC header parser reading the fixed-length fields (XID, RPC Version) and then invoking the `auth` function to read the variable-length credentials. The parser then populates the `RpcMessage` struct (defined here) with the Program, Version, Procedure, and the parsed credentials. This struct is then passed up the stack to determine if the request should be accepted (based on `cred`) and which internal trait implementation (NFS, MOUNT, etc.) should handle the `procedure`.

Inside the system, the following things happen and they use this module:
- **Security Enforcement**: The `auth` function uses `vec_max_size` with `MAX_AUTH_SIZE` to strictly limit the amount of memory allocated for authentication data. This is crucial for preventing a malicious client from sending a massive authentication blob to exhaust server memory.
- **Protocol Dispatching**: The `RpcMessage` struct aggregates the `program`, `version`, and `procedure` fields. While this module does not parse them itself (it only defines the struct), it provides the standard container that higher-level parsers fill. This allows the dispatcher to pattern match on `RpcMessage` to route the request.

Without this module, the parsing logic for authentication—which involves specific size limits and enum mapping—would be duplicated in every place an RPC header is parsed. Furthermore, the definition of `RpcMessage` would be scattered, breaking the contract between the network layer and the service dispatch layer.