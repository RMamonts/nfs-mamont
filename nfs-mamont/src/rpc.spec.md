<!-- SPEC_HASH: 68e941a72c51a01628163c02e4e4ce86ec6ee453d5170291443b5292b6c1188b -->
# Module Specification

Module: nfs_mamont::rpc
Rust File: /home/yarovoy/Documents/yadro/projects/oss/github/nfs-mamont/nfs-mamont/src/rpc.rs

---

## 1. Dependencies

From the code analysis, the following external crates and standard library modules are used:

- **`std::io`**: Used to wrap standard I/O errors within the module's custom `Error` enum (`Error::IO`). This allows the module to propagate low-level stream or network failures.
- **`std::string::FromUtf8Error`**: Used to wrap UTF-8 conversion errors within the `Error` enum (`Error::IncorrectString`). This is relevant when parsing string fields from the RPC protocol that must be valid UTF-8.
- **`num_derive` (and implicitly `num_traits`)**: Used to derive `FromPrimitive` and `ToPrimitive` traits on enums. This is critical for the serialization and deserialization process, allowing the conversion between integer discriminants found in the binary RPC protocol and the semantic Rust enum variants (e.g., `AcceptStat`, `AuthStat`).

*Assumption:* Since specifications for dependencies were not provided, it is assumed that `num_traits` provides the `FromPrimitive` and `ToPrimitive` traits used by the derived implementations, and that standard library types behave as per their official Rust documentation.

---

## 2. Mechanics

This module acts as a data definition layer for the ONC RPC (Remote Procedure Call) protocol. It does not contain executable logic functions but defines the types and constants necessary for encoding, decoding, and handling RPC messages.

Intent:
- To provide a strongly-typed representation of the ONC RPC protocol states, errors, and authentication data structures.
- To centralize protocol constants (like version numbers and size limits).
- To define a comprehensive error type that aggregates various failure modes encountered during RPC processing.

Inputs:
- N/A (This module defines types; it does not consume inputs directly).

Outputs:
- Type definitions (`struct`, `enum`) and constants.

Steps:
1. Define protocol constants (`RPC_VERSION`, `MAX_AUTH_SIZE`).
2. Define enumerations for protocol states (`AcceptStat`, `AuthStat`, `RpcBody`, `ReplyBody`, `AuthFlavor`, `RejectedReply`), deriving integer conversion traits to map to wire formats.
3. Define structures for data payloads (`OpaqueAuth`, `VersionMismatch`).
4. Define a unified `Error` enum to represent all potential failure conditions during parsing or handling.

Edge Cases:
- N/A (Static definitions).

Complexity:
- Time: N/A
- Space: N/A

Determinism:
- Deterministic (Type definitions are static).

---

## 3. Dependency Mechanics

Since no dependency specifications were provided, the following standard library and external crate mechanics are inferred as relevant:

- **`std::io::Error`**: Represents generic I/O errors. The `Error::IO` variant wraps this, allowing RPC operations to fail due to underlying read/write issues.
- **`std::string::FromUtf8Error`**: Represents errors when a byte vector is not valid UTF-8. The `Error::IncorrectString` variant wraps this, used when string fields in RPC messages fail validation.
- **`num_traits::FromPrimitive`**: Provides the mechanism to convert an integer (e.g., read from a stream) into an enum variant (like `AcceptStat`).
- **`num_traits::ToPrimitive`**: Provides the mechanism to convert an enum variant back into its integer representation for transmission.

---

## 4. Data Model

Entities:
- **`AcceptStat`**: Enum indicating the acceptance status of an RPC call (e.g., `Success`, `ProgUnavail`).
- **`AuthStat`**: Enum indicating the status of an authentication attempt (e.g., `Ok`, `BadCred`, `RejectedVerf`).
- **`RpcBody`**: Enum distinguishing between the two high-level RPC message types: `Call` and `Reply`.
- **`ReplyBody`**: Enum distinguishing between an accepted reply (`MsgAccepted`) and a denied reply (`MsgDenied`).
- **`AuthFlavor`**: Enum identifying the authentication flavor (e.g., `None`, `Sys`, `RpcSecGss`).
- **`RejectedReply`**: Enum identifying why a reply was rejected (`RpcMismatch`, `AuthError`).
- **`OpaqueAuth`**: Struct containing an `AuthFlavor` and a `Vec<u8>` body representing raw authentication data.
- **`VersionMismatch`**: Struct containing `low` and `high` `u32` fields, representing the range of supported versions when a mismatch occurs.
- **`Error`**: Enum representing all possible errors, including IO, parsing, and protocol-specific errors.

Relations:
- `Error` → `io::Error` (Composition via `Error::IO`)
- `Error` → `FromUtf8Error` (Composition via `Error::IncorrectString`)
- `Error` → `VersionMismatch` (Composition via `Error::RpcVersionMismatch` and `Error::ProgramVersionMismatch`)
- `Error` → `AuthStat` (Composition via `Error::Auth`)
- `OpaqueAuth` → `AuthFlavor` (Composition)

Global Invariants:
- `RPC_VERSION` is always `2`.
- `MAX_AUTH_SIZE` is always `400`.
- Enum discriminants are fixed to specific integer values (e.g., `AcceptStat::Success` is `0`).

---

## 5. Error Model

Error Types:
- **`Error`**: A comprehensive enum covering:
    - `MaxElemLimit`: Internal limits exceeded.
    - `IO(io::Error)`: Wrapper for I/O errors.
    - `EnumDiscMismatch`: Invalid enum discriminant encountered.
    - `IncorrectString(FromUtf8Error)`: Invalid UTF-8 data.
    - `ImpossibleTypeCast`: Type casting failure.
    - `BadFileHandle`: Invalid file handle encountered.
    - `MessageTypeMismatch`: Unexpected message type.
    - `RpcVersionMismatch(VersionMismatch)`: RPC protocol version mismatch.
    - `Auth(AuthStat)`: Authentication failure.
    - `ProgramMismatch`: Requested program not available.
    - `ProcedureMismatch`: Requested procedure not available.
    - `ProgramVersionMismatch(VersionMismatch)`: Program version mismatch.

Error Propagation Strategy:
- Custom enum (`Error`). The module defines its own error type to aggregate specific protocol errors with general I/O and parsing errors.

Recoverability:
- Context-dependent. Errors like `IO` or `IncorrectString` are likely fatal for the current operation. `VersionMismatch` errors might allow the client to retry with a different version if supported.

Panics:
- Allowed: No (No code in this module triggers panics; it only defines types).

---

## 6. Traits

The module implements the following external traits (via `derive` macros):

- **`ToPrimitive`**: Implemented on `AcceptStat`, `AuthStat`, `RpcBody`, `AuthFlavor`. Allows converting the enum to an integer.
- **`FromPrimitive`**: Implemented on `AcceptStat`, `AuthStat`, `RpcBody`, `AuthFlavor`. Allows creating an enum from an integer.
- **`Debug`**: Implemented on `AuthStat`, `AuthFlavor`, `OpaqueAuth`, `VersionMismatch`, `Error`.
- **`PartialEq`**: Implemented on `AuthStat`, `AuthFlavor` (and `OpaqueAuth` in test cfg).
- **`PartialOrd`**: Implemented on `AuthStat`.
- **`Clone`**: Implemented on `AuthFlavor`, `OpaqueAuth`.

---

## 7. Overview

This module is used in order to establish the type system foundation for an ONC RPC implementation, likely serving an NFS (Network File System) protocol stack (indicated by the crate name `nfs_mamont`). It defines the "vocabulary" of the protocol—mapping integer codes found in network packets to meaningful Rust types like `AcceptStat` or `AuthFlavor`.

This system contains a set of data structures that strictly adhere to the ONC RPC specification. It separates the definition of protocol states and errors from the logic of serialization or network handling, allowing other modules to depend on these concrete types without knowing the details of the wire format.

A typical usage scenario of the system involves a network handler receiving a raw byte stream. The handler uses the `FromPrimitive` traits implemented on the enums in this module to interpret specific bytes as `AcceptStat` or `AuthStat`. If the stream indicates an authentication failure, the handler constructs an `Error::Auth` variant containing the specific `AuthStat`. If the stream contains opaque authentication data, it is stored in an `OpaqueAuth` struct.

Inside the system, the following things happen and they use:
- **Protocol Negotiation**: Uses `RpcBody` and `ReplyBody` to distinguish between calls and replies.
- **Error Handling**: Uses the `Error` enum to unify reporting of I/O issues (`std::io`), encoding issues (`FromUtf8Error`), and protocol logic errors (`AuthStat`, `VersionMismatch`).
- **Authentication**: Uses `AuthFlavor` and `OpaqueAuth` to describe and transport credentials.