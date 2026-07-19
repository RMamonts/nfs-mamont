<!-- SPEC_HASH: e492a57faef6dd68b7d71785af0030a2f75cb2a99c90962ee4dd0b803b5b541e -->
# Module Specification

Module: nfs_mamont::serializer::server::nfs::fs_stat
Rust File: src/serializer/server/nfs/fs_stat.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **std::io**: Used to provide the `Write` trait, which defines the destination sink for the XDR bytes, and `io::Result` for handling I/O errors during serialization.
- **crate::serializer::files**: Used to access the `file_attr` function. This is necessary to serialize the `root_attr` field (file attributes of the root directory) which is part of both the success and failure response structures.
- **crate::serializer**: Used to access low-level XDR serialization primitives: `option` (for serializing optional fields like `root_attr`), `u32` (for serializing the `invarsec` field), and `u64` (for serializing byte and file count fields).
- **crate::vfs::fs_stat**: Used as the source of the domain-specific data types `Success` and `Fail`. These structs contain the file system statistics and error information that need to be converted into the wire format.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To transform the internal VFS representation of the NFSv3 `FSSTAT` procedure results (`Success` and `Fail`) into the binary XDR (External Data Representation) format required for network transmission.
- To map the specific fields of the `Success` struct (distinguishing between total, free, and available resources) to the corresponding `FSSTAT3resok` XDR structure defined in RFC 1813.

Inputs:
- `dest`: A mutable reference to a type implementing `std::io::Write` (e.g., a network buffer).
- `arg`: Either a `fs_stat::Success` struct (containing file system metrics) or a `fs_stat::Fail` struct (containing error details).

Outputs:
- `io::Result<()>`: Indicates successful serialization of the structure into the destination or an error if writing fails.

Steps:
1. **Success Serialization (`result_ok`)**:
   - Serializes the `root_attr` field using the `option` helper, which internally calls `file_attr` if the attribute is present.
   - Serializes `total_bytes` using the `u64` primitive.
   - Serializes `free_bytes` using the `u64` primitive.
   - Serializes `available_bytes` using the `u64` primitive.
   - Serializes `total_files` using the `u64` primitive.
   - Serializes `free_files` using the `u64` primitive.
   - Serializes `available_files` using the `u64` primitive.
   - Serializes `invarsec` using the `u32` primitive.
2. **Failure Serialization (`result_fail`)**:
   - Serializes the `root_attr` field using the `option` helper, which internally calls `file_attr` if the attribute is present.

Edge Cases:
- **Optional Attributes**: The `root_attr` field is wrapped in an `Option` in both `Success` and `Fail`. The `option` serializer handles the logic of writing a boolean discriminant followed by the data only if the attribute exists.

Complexity:
- Time: O(1). The functions perform a fixed sequence of writes corresponding to the fields of the structs.
- Space: O(1) auxiliary space (streaming directly to the writer), excluding the buffer managed by the `Write` implementation.

Determinism:
- Deterministic

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `crate::serializer`**:
 - **`u32` and `u64`**: These functions are essential for writing the integer fields (bytes, file counts, `invarsec`) in the correct Big Endian byte order required by XDR.
 - **`option`**: This function is used to serialize the `root_attr` field. It handles the XDR encoding for optional data (a boolean indicating presence followed by the data itself), ensuring the protocol's weak cache consistency requirements are met.
- **From `crate::serializer::files`**:
 - **`file_attr`**: This function is invoked by the `option` serializer to convert the `file::Attr` struct (if present) into the `fattr3` XDR format. This allows the `FSSTAT` response to include up-to-date metadata about the root directory.
- **From `crate::vfs::fs_stat`**:
 - **`Success` and `Fail`**: These structs define the data contract. The `Success` struct specifically provides the distinction between "free" and "available" resources (e.g., `free_bytes` vs `available_bytes`), which this module transmits faithfully to the client to accurately reflect file system reservations.

---

## 4. Data Model

Entities:
- **Serializers**: Stateless functions `result_ok` and `result_fail` that act as converters.
- **XDR Structures**: The implicit binary layouts defined by RFC 1813 for `FSSTAT3resok` and `FSSTAT3resfail`.

Relations:
- **Mapping**: `vfs::fs_stat::Success` maps to XDR `FSSTAT3resok`. `vfs::fs_stat::Fail` maps to XDR `FSSTAT3resfail`.
- **Composition**: `result_ok` composes `option`, `file_attr`, `u64`, and `u32`.

Global Invariants:
- The order of fields written by `result_ok` must strictly follow the NFSv3 specification: `obj_attributes` (root_attr), `tbytes`, `fbytes`, `abytes`, `tfiles`, `ffiles`, `afiles`, `invarsec`.

## 5. Error Model

Error Types:
- `std::io::Error`

Error Propagation Strategy:
- Errors are propagated using the `?` operator. This includes I/O errors from the underlying `Write` implementation.

Recoverability:
- Recoverable. The functions return `Result`, allowing the caller to handle serialization failures (e.g., by aborting the RPC response).

Panics:
- Allowed: No
- Conditions: The code does not explicitly panic; it relies on the `Write` trait and `Result` handling.

---

## 6. Traits

List which external traits this module implements:
- None. This module defines free-standing functions and does not implement traits for its own types.

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependencies. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here you MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to serialize the response for the NFSv3 `FSSTAT` procedure, which allows clients to query dynamic information about a file system's capacity and usage. The system contains an NFSv3 server that must communicate storage metrics (such as total disk space, free space, and available file slots) to clients over the network. The internal VFS layer calculates these metrics and returns them as Rust structs (`vfs::fs_stat::Success` or `vfs::fs_stat::Fail`). However, these internal structures cannot be sent directly because they do not conform to the XDR standard required by the NFS protocol.

A typical usage scenario of the system involves a client running a command like `df` (disk free) to check available space on a mounted NFS share. The server receives the `FSSTAT` request, the VFS retrieves the current statistics from the backend storage, and the system then invokes the `result_ok` function from this module. This function translates the fields—such as `total_bytes` and `available_bytes`—into a sequence of bytes conforming to the `FSSTAT3resok` structure defined in RFC 1813. Inside the system, this translation is critical for accurate reporting; specifically, the module ensures that the distinction between "free" resources (total free on disk) and "available" resources (free space usable by non-privileged users) is preserved on the wire. Without this module, the server would be unable to encode these statistics, preventing the client from knowing if the file system is full or if it has sufficient space to write new files.