<!-- SPEC_HASH: 8da37f59353ec8f0888685996437aa01964415ea36e71aa5c64e07a71ce75fed -->
# Module Specification

Module: nfs_mamont::serializer::server::nfs::fs_info
Rust File: src/serializer/server/nfs/fs_info.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`std::io`**: Used to provide the `Write` trait, which defines the interface for the destination byte stream where the XDR encoded data will be written.
- **`crate::serializer::files`**: Used to import `file_attr` and `nfs_time`. These functions are necessary to serialize the complex nested structures (`file::Attr` and `file::Time`) contained within the `fs_info::Success` and `fs_info::Fail` structs.
- **`crate::serializer`**: Used to import primitive serialization helpers `option`, `u32`, and `u64`. These are used to handle optional fields and encode integer values according to the XDR standard (Big Endian).
- **`crate::vfs::fs_info`**: Used to import the `Success` and `Fail` structs. These are the source data types representing the result of a VFS `FSINFO` operation that need to be converted into the wire format.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To convert the internal representation of the NFSv3 `FSINFO` procedure results (both success and failure cases) into the standardized XDR (External Data Representation) binary format.
- To map the specific fields of the VFS `Success` struct (transfer sizes, properties, time delta) to the corresponding fields in the `FSINFO3resok` XDR structure.
- To map the `Fail` struct to the `FSINFO3resfail` XDR structure, specifically handling the post-operation attributes.

Inputs:
- `dest`: A mutable reference to a type implementing `std::io::Write`, acting as the byte sink.
- `arg`: Either a `fs_info::Success` struct (for successful operations) or a `fs_info::Fail` struct (for failed operations).

Outputs:
- `io::Result<()>`: Indicates success or failure of the write operation to the destination.

Steps:
1. **Serialization of Success (`result_ok`)**:
   - The function serializes the `root_attr` field using the `option` helper. If present, it delegates to `file_attr` to write the file attributes.
   - It serializes a sequence of `u32` integers representing transfer limits and preferences: `read_max`, `read_pref`, `read_mult`, `write_max`, `write_pref`, `write_mult`, and `read_dir_pref`.
   - It serializes `max_file_size` as a `u64`.
   - It serializes `time_delta` using the `nfs_time` helper.
   - It serializes the `properties` bitset by converting it to a `u32` via the `bits()` method.
2. **Serialization of Failure (`result_fail`)**:
   - The function serializes the `root_attr` field using the `option` helper. If present, it delegates to `file_attr`.
   - Note: The error status code itself is not serialized by this function; it is expected to be handled by the calling RPC layer (as the union discriminant).

Edge Cases:
- **Missing Attributes**: If `arg.root_attr` is `None` in either `Success` or `Fail`, the `option` helper writes a boolean `false` (or equivalent XDR representation for absence) and skips the attribute serialization.
- **Write Failures**: Any error reported by the underlying `Write` implementation or the helper serializers is propagated immediately via the `?` operator.

Complexity:
- Time: O(1). The number of fields written is constant and fixed by the NFSv3 protocol specification.
- Space: O(1) auxiliary space (excluding the buffer managed by the `Write` implementation).

Determinism:
- Deterministic

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::serializer::files`**:
 - **`file_attr`**: Used to serialize the `root_attr` field. This mechanism handles the complex layout of file attributes (type, mode, size, timestamps, etc.) defined in RFC 1813.
 - **`nfs_time`**: Used to serialize the `time_delta` field. This mechanism handles the conversion of the VFS `Time` type into the XDR `nfstime3` structure (seconds and nanoseconds).

- **From `nfs_mamont::serializer`**:
 - **`option`**: Used to serialize the optional `root_attr` fields in both `Success` and `Fail`. This mechanism handles the XDR union logic for optional data (writing a discriminant followed by the value if present).
 - **`u32` and `u64`**: Used to serialize the scalar fields representing transfer sizes, preferences, and the properties bitmask. These mechanisms ensure correct Big Endian byte ordering.

- **From `nfs_mamont::vfs::fs_info`**:
 - **`Success` and `Fail`**: These structs define the data contract. The `Success` struct aggregates the server's capabilities (buffer sizes, features), while `Fail` aggregates the error context (post-operation attributes).

---

## 4. Data Model

Entities:
- **`result_ok` Function**: A serializer that maps `vfs::fs_info::Success` to the XDR `FSINFO3resok` structure.
- **`result_fail` Function**: A serializer that maps `vfs::fs_info::Fail` to the XDR `FSINFO3resfail` structure.

Relations:
- **Mapping**: `result_ok` maps fields from `vfs::fs_info::Success` (e.g., `read_max`) to specific XDR integer fields.
- **Mapping**: `result_fail` maps the `root_attr` from `vfs::fs_info::Fail` to the `post_op_attr` field in the XDR failure response.

Global Invariants:
- The output byte stream must conform to the NFSv3 XDR specification for `FSINFO3resok` and `FSINFO3resfail` as defined in RFC 1813.
- The order of fields in `result_ok` is strictly defined: attributes, read sizes, write sizes, directory read preference, max file size, time delta, and properties.

## 5. Error Model

Error Types:
- **`std::io::Error`**: The only error type produced directly by this module.

Error Propagation Strategy:
- **Propagation**: Errors are propagated using the `?` operator from the underlying serialization helpers (`option`, `u32`, `file_attr`, etc.) or the `Write` trait implementation.

Recoverability:
- **Recoverable**: The functions return `io::Result<()>`, allowing the caller (e.g., the RPC layer) to catch the error and potentially abort the connection or send a generic RPC error.

Panics:
- **Allowed**: No
- **Conditions**: The code consists entirely of calls to fallible functions (`?`) and does not contain any `unwrap`, `expect`, or `panic!` calls.

---

## 6. Traits

List which external traits this module implements:
- None. This module defines free-standing serialization functions.

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to serialize the results of the NFSv3 `FSINFO` procedure into the XDR wire format required for network transmission. The system contains an NFSv3 server where the Virtual File System (VFS) layer handles the logic of determining file system capabilities (such as maximum read/write sizes, supported features like symlinks, and time granularity). These capabilities are represented internally by the `vfs::fs_info::Success` and `vfs::fs_info::Fail` structures. However, these internal Rust structures cannot be sent directly to the client.

A typical usage scenario of the system involves a client connecting to the server and immediately issuing an `FSINFO` request to discover the server's limits and features. The VFS layer processes this request and returns a `Success` struct containing parameters like `read_max` and `properties`. The system then invokes the `result_ok` function from this module. This function translates the internal fields into a specific sequence of bytes defined by the NFSv3 protocol (RFC 1813). For example, it ensures that the `properties` bitset is written as a 32-bit integer and that `time_delta` is encoded as two 32-bit integers (seconds and nanoseconds).

Inside the system, this module serves as the specific adapter for the `FSINFO` procedure. While the generic `serializer` and `serializer::files` modules provide the tools to write integers, options, and file attributes, this module dictates *exactly* how those tools are applied to construct the `FSINFO3resok` and `FSINFO3resfail` messages. Without this module, the server would lack the logic to format this specific procedure's response, preventing clients from correctly negotiating transfer sizes or understanding file system properties.