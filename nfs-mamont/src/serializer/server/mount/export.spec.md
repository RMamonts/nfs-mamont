<!-- SPEC_HASH: 18786c2325aecb5c27a35e3c8b4b80fef3d4d2af98862fccd173894cfbc1e75e -->
# Module Specification

Module: nfs_mamont::serializer::server::mount::export
Rust File: src/serializer/server/mount/export.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`std::io::Write`**: Used as the destination trait for the serialized byte stream. It allows the functions to write to any target that implements `Write`, such as network sockets or memory buffers.
- **`crate::consts::mount::MOUNT_HOST_NAME_LEN`**: Used to define the maximum allowed byte length for a hostname string during serialization. This constant is passed to `string_max_size` to enforce protocol limits.
- **`crate::mount::ExportEntry`**: Used as the input data structure for `export_entry`. It contains the directory path and the list of client hostnames that need to be serialized.
- **`crate::mount::export::Success`**: Used as the input data structure for `result_ok`. It contains the vector of all `ExportEntry` objects representing the server's export list.
- **`crate::serializer::files::file_path`**: Used to serialize the `directory` field of `ExportEntry`. It handles the specific encoding requirements for filesystem paths (e.g., UTF-8 conversion, length validation).
- **`crate::serializer::bool`**: Used to serialize boolean values. In this module, it is specifically used to encode the "next pointer" in the XDR linked list structures (indicating whether another item follows).
- **`crate::serializer::string_max_size`**: Used to serialize the hostname strings. It ensures the string is written as XDR variable-length data with padding and validates that the length does not exceed `MOUNT_HOST_NAME_LEN`.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To convert high-level Rust data structures representing the MOUNT protocol's EXPORT response into the binary XDR (External Data Representation) format required for network transmission.
- To implement the specific linked list encoding defined in RFC 1813 for `exportnode` (list of exports) and `groupnode` (list of groups/hostnames), where the presence of a next node is indicated by a boolean discriminant.

Inputs:
- `dest`: A mutable reference to a type implementing `std::io::Write`.
- `arg`: Either a `mount::ExportEntry` (for `export_entry`) or an `export::Success` (for `result_ok`).

Outputs:
- `io::Result<()>`: Indicates success or an I/O error during writing.

Steps:
1. **`export_entry` Serialization**:
   - The function first serializes the `directory` field of the `ExportEntry` by calling `file_path`.
   - It then iterates over the `names` vector (list of hostnames).
   - For each hostname in the vector, it writes a boolean `true` (indicating that a group node follows) and then serializes the hostname string using `string_max_size` with the limit `MOUNT_HOST_NAME_LEN`.
   - After the loop, it writes a boolean `false` to signify the end of the linked list of hostnames (null terminator).

2. **`result_ok` Serialization**:
   - The function iterates over the `exports` vector contained in the `Success` struct.
   - For each `ExportEntry`, it writes a boolean `true` (indicating that an export node follows) and then recursively calls `export_entry` to serialize the details of that export.
   - After the loop, it writes a boolean `false` to signify the end of the linked list of exports (null terminator).

Edge Cases:
- **Empty Lists**: If the `names` vector in `ExportEntry` is empty, the function writes `bool(dest, false)` immediately after the directory path, resulting in a list with zero elements.
- **Empty Exports**: If the `exports` vector in `Success` is empty, the function writes `bool(dest, false)` immediately, resulting in an empty export list.

Complexity:
- Time: O(N), where N is the total number of hostnames across all export entries plus the number of export entries themselves. The function visits every element exactly once.
- Space: O(1) auxiliary space (excluding the buffer managed by the `Write` implementation).

Determinism:
- Deterministic

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::serializer`**:
    - **`bool`**: This primitive is critical for implementing the XDR linked list structure. The module relies on it to write the discriminant (`true`/`false`) that determines if the list continues or terminates.
    - **`string_max_size`**: This primitive is used to enforce the protocol constraint on hostname length. It handles the XDR encoding of the string (length prefix + bytes + padding) and returns an error if the string is too long.
- **From `nfs_mamont::serializer::files`**:
    - **`file_path`**: This function is used to serialize the directory path associated with an export. It abstracts away the conversion of the internal `PathBuf` type to the XDR string format required by the protocol.
- **From `nfs_mamont::mount`**:
    - **`ExportEntry`**: This struct defines the data layout that `export_entry` is responsible for serializing. It couples the directory path with the list of allowed clients.
- **From `nfs_mamont::mount::export`**:
    - **`Success`**: This struct acts as the container for the top-level list of exports. `result_ok` iterates over its `exports` field to generate the response.

---

## 4. Data Model

Entities:
- **`export_entry` function**: Represents the serializer for the XDR `groupnode` structure. It maps a `mount::ExportEntry` to a binary stream consisting of a path followed by a linked list of hostnames.
- **`result_ok` function**: Represents the serializer for the XDR `exportnode` structure. It maps an `export::Success` to a binary stream consisting of a linked list of `export_entry` nodes.

Relations:
- **`result_ok` → `export_entry` (1:N)**: `result_ok` calls `export_entry` for every item in the export list.
- **`export_entry` → `file_path` (1:1)**: `export_entry` calls `file_path` once to serialize the directory.
- **`export_entry` → `string_max_size` (1:N)**: `export_entry` calls `string_max_size` for every hostname in the list.

Global Invariants:
- The output stream must conform to the XDR standard linked list encoding defined in RFC 1813, Section 2.2.4 (Linked Lists).
- The length of any serialized hostname must not exceed `MOUNT_HOST_NAME_LEN`.

## 5. Error Model

Error Types:
- **`std::io::Error`**: This is the only error type returned. It can originate from the underlying `Write` implementation (e.g., broken pipe) or from validation logic within the dependency serializers (e.g., `string_max_size` returning `InvalidInput` if a hostname is too long).

Error Propagation Strategy:
- **Propagation**: The module uses the `?` operator to propagate errors immediately. If any serialization step (writing a bool, a string, or a path) fails, the function returns the error to the caller without writing further data.

Recoverability:
- **Recoverable**: The functions return `Result`, allowing the caller (likely the RPC layer) to handle the error (e.g., by closing the connection or logging the failure).

Panics:
- **Allowed**: No
- **Conditions**: The code does not perform any operations that could panic (such as unwrapping `Option` or indexing out of bounds). It relies entirely on the `Write` trait and `Result` propagation.

---

## 6. Traits

List which external traits this module implements:
- None. This module defines free-standing serialization functions.

---

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here you MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to **serialize the server's response to the MOUNT protocol's EXPORT procedure (Procedure 5)** into the binary format expected by NFS clients. The system contains an NFS server that maintains a list of exported filesystems (directories) and the clients permitted to access them. When a client requests this list, the server's internal logic generates a high-level Rust structure (`export::Success`), but this structure cannot be sent directly over the network.

A typical usage scenario of the system involves a client connecting to the server and invoking the MOUNT EXPORT RPC to discover available shares. The server retrieves the export list from its backend, resulting in a `Success` struct containing a vector of `ExportEntry` items. The system then invokes the `result_ok` function from this module, passing the network buffer and the `Success` struct. The function translates the vector of exports into an XDR linked list of `exportnode` structures. For each export, it calls `export_entry`, which further translates the list of allowed hostnames into an XDR linked list of `groupnode` structures.

Inside the system, the following things happen and they use this module:
1. **Linked List Encoding**: The MOUNT protocol uses linked lists rather than fixed arrays for variable-length data. This module is responsible for writing the boolean discriminants (`true` for "next item exists", `false` for "end of list") that define these linked lists in the byte stream.
2. **Protocol Enforcement**: By passing `MOUNT_HOST_NAME_LEN` to `string_max_size`, the module ensures that the server never sends a hostname that exceeds the size limit defined by the NFS specification, preventing protocol violations on the client side.
3. **Data Transformation**: It bridges the gap between the server's internal representation (Rust `Vec`s and `PathBuf`s) and the wire format (XDR bytes), ensuring that data like directory paths and hostnames are correctly encoded with padding and byte ordering.

Without this module, the server would be unable to communicate its export configuration to clients, effectively breaking the discovery mechanism of the NFS MOUNT protocol.