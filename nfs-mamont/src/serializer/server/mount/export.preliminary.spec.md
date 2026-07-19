<!-- SPEC_HASH: 18786c2325aecb5c27a35e3c8b4b80fef3d4d2af98862fccd173894cfbc1e75e -->
# Module Specification

Module: nfs_mamont::serializer::server::mount::export
Rust File: src/serializer/server/mount/export.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **std::io / std::io::Write**: Used to define the output sink trait (`Write`) that the serialization functions write into. This allows the serializer to write to any buffer or stream that implements the standard `Write` trait (e.g., `Vec<u8>`, TCP stream).
- **crate::consts::mount**: Used to obtain the `MOUNT_HOST_NAME_LEN` constant. This constant is passed to the string serializer to enforce the maximum length constraint for hostnames as defined by the MOUNT protocol.
- **crate::mount**: Used to import the domain types `mount::ExportEntry` and `export::Success`. These structures represent the in-memory data model of the server's export table that needs to be serialized.
- **crate::serializer::files**: Used to access the `file_path` function. This function is responsible for encoding the directory path field of an export entry according to the specific XDR rules for filesystem paths.
- **crate::serializer**: Used to access primitive encoding functions `bool` and `string_max_size`. These are used to encode the boolean discriminators for XDR linked lists and the hostname strings respectively.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To convert the server's internal representation of filesystem exports (Rust structs) into the binary XDR (External Data Representation) format defined by the NFS MOUNT protocol. Specifically, it handles the serialization of linked lists structures (`exportnode` and `groupnode`) used in the `MOUNTPROC_EXPORT` response.

Inputs:
- `dest`: A mutable reference to a type implementing the `Write` trait, acting as the byte sink.
- `arg`: Either a `mount::ExportEntry` (for `export_entry`) or an `export::Success` (for `result_ok`), containing the data to be serialized.

Outputs:
- `io::Result<()>`: An empty result type indicating success or an I/O error if writing to the destination fails.

Steps:
1. **`export_entry` serialization**:
   - Calls `file_path` to serialize the `directory` field of the `ExportEntry`.
   - Iterates over the `names` vector (list of hostnames).
   - For each hostname, writes a boolean `true` (indicating the presence of a next node in the linked list) followed by the hostname string serialized via `string_max_size` (constrained by `MOUNT_HOST_NAME_LEN`).
   - After processing all hostnames, writes a boolean `false` to signify the end of the linked list (nil terminator).
2. **`result_ok` serialization**:
   - Iterates over the `exports` vector inside the `Success` struct.
   - For each `ExportEntry`, writes a boolean `true` (indicating the presence of a next node) followed by a recursive call to `export_entry` to serialize the entry's content.
   - After processing all entries, writes a boolean `false` to signify the end of the export list.

Edge Cases:
- **Empty Lists**: If the `names` vector in `ExportEntry` is empty, the function immediately writes `false` after the directory path, creating a valid empty linked list. Similarly for an empty `exports` vector in `result_ok`.

Complexity:
- Time: O(N), where N is the total number of characters in all directory paths and hostnames plus the number of entries (due to the linear traversal of vectors).
- Space: O(1) auxiliary space (excluding the output buffer managed by `dest`).

Determinism:
- Deterministic

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `crate::serializer`**: The `bool` function is used to implement the XDR linked list encoding pattern (discriminated unions), where `true` indicates a valid node follows and `false` indicates a null terminator. The `string_max_size` function ensures that the serialized string does not exceed the protocol-defined limit, returning an error if the input is too long.
- **From `crate::serializer::files`**: The `file_path` function handles the specific encoding requirements for filesystem paths, which may differ from generic strings (e.g., specific handling of separators or length limits).
- **From `crate::consts::mount`**: The `MOUNT_HOST_NAME_LEN` constant provides the specific size limit required by the MOUNT protocol to ensure interoperability and prevent buffer overflows on the client side.

---

## 4. Data Model

Entities:
- **`mount::ExportEntry`**: A structure containing a directory path (`file::Path`) and a list of client hostnames (`Vec<HostName>`). In the context of serialization, this maps to the XDR `groupnode`.
- **`export::Success`**: A structure containing a vector of `ExportEntry` items. In the context of serialization, this maps to the XDR `exportnode` linked list.

Relations:
- **Aggregation**: `export::Success` aggregates multiple `mount::ExportEntry` instances.
- **Composition**: `mount::ExportEntry` composes a `file::Path` and multiple `HostName` strings.

Global Invariants:
- The output stream must represent a valid XDR encoding of the MOUNT protocol export list. Specifically, the linked lists must be properly terminated with a boolean `false`.
- Hostname strings must not exceed `MOUNT_HOST_NAME_LEN` bytes; otherwise, the serialization will fail via the `string_max_size` helper.

## 5. Error Model

Error Types:
- `std::io::Error`: Propagated from the underlying `Write` trait implementation or from the helper serialization functions (e.g., if a string exceeds the maximum size).

Error Propagation Strategy:
- Propagation (using the `?` operator). Errors are not handled or modified within this module; they are passed directly to the caller.

Recoverability:
- Not applicable at this layer. If an I/O error or size constraint violation occurs, the serialization process aborts, and the error is returned to the caller for handling (e.g., closing the connection).

Panics:
- Allowed: No
- Conditions: This module performs no explicit panics. It relies on safe Rust abstractions (`Write` trait) and helper functions that return `Result`.

---

## 6. Traits

List which external traits this module implements:
- None. This module defines free-standing functions and does not implement traits for types.

---

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here you MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to serialize the server's response to the MOUNT protocol's `EXPORT` procedure. The MOUNT protocol requires that the list of exported filesystems be returned as a specific type of linked list structure in XDR format (an `exportnode` containing `groupnode`s). This system contains the logic to translate the high-level Rust representation of these exports (vectors of structs) into the low-level byte stream required by the network protocol. A typical usage scenario of the system involves an NFS client requesting the list of exported directories from the server. The server generates a `Success` struct containing the export data. Inside the system, the following things happen and they use this module: the `result_ok` function is called with the `Success` struct. It iterates through the exports, and for each one, it calls `export_entry`. `export_entry` writes the directory path and then iterates through the allowed hostnames, writing them as a linked list. This process ensures that the complex, nested linked list structures defined in the NFS MOUNT RFC are correctly flattened into a byte sequence that can be transmitted over the network, allowing the client to discover which directories are available for mounting.