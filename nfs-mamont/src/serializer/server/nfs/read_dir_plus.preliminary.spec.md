<!-- SPEC_HASH: 5b0ae14207d29488a6c6300f8469ea5de39844f26599b141e864f3a91e3cb7e5 -->
# Module Specification

Module: nfs_mamont::serializer::server::nfs::read_dir_plus
Rust File: src/serializer/server/nfs/read_dir_plus.rs

---

## 1. Dependencies

- **`std::io` and `std::io::Write`**: Used to define the output sink trait. The module writes bytes to any type implementing `Write`, such as network streams or buffers.
- **`crate::serializer::files`**: Provides specific XDR serializers for file system entities (`file_attr`, `file_handle`, `file_name`). This module delegates the serialization of complex sub-structures to these functions.
- **`crate::serializer`**: Provides generic XDR primitive serializers (`array`, `bool`, `option`, `u64`). These are used to encode fundamental data types and construct the XDR structure (e.g., optional fields, fixed-length arrays).
- **`crate::vfs::read_dir_plus`**: Provides the domain-specific data structures (`Entry`, `Success`, `Fail`) representing the result of a VFS directory read operation. This module consumes these structures to generate the network response.

---

## 2. Mechanics

### Mechanism 1: Entry Serialization
**Intent**: Convert a single directory entry structure into its XDR representation.

**Inputs**:
- `dest`: A mutable reference to a type implementing `Write`.
- `entry`: An `Entry` struct from `vfs::read_dir_plus`.

**Outputs**:
- Writes XDR encoded bytes to `dest`.

**Steps**:
1. Serialize the `file_id` as a 64-bit unsigned integer using `u64`.
2. Serialize the `file_name` using the delegated `file_name` serializer.
3. Serialize the `cookie` as a 64-bit unsigned integer using `u64` (accessed via `.raw()`).
4. Serialize the optional `file_attr` using the `option` helper. If present, it delegates to `file_attr`.
5. Serialize the optional `file_handle` using the `option` helper. If present, it delegates to `file_handle`.

**Edge Cases**:
- If `file_attr` or `file_handle` are `None`, the `option` serializer writes a boolean `false` and skips the data.

**Complexity**:
- Time: O(1) relative to the number of entries (per entry).
- Space: O(1) auxiliary space.

**Determinism**: Deterministic.

---

### Mechanism 2: Linked List Serialization
**Intent**: Serialize a vector of entries into an XDR linked list structure.

**Inputs**:
- `dest`: A mutable reference to a type implementing `Write`.
- `list`: A `Vec<Entry>`.

**Outputs**:
- Writes XDR encoded bytes representing a linked list to `dest`.

**Steps**:
1. Iterate over each entry in the `list`.
2. For each entry, write a boolean `true` to indicate the presence of a next node.
3. Call `entry` to serialize the current entry's data.
4. After the loop, write a boolean `false` to signify the end of the list (nil terminator).

**Edge Cases**:
- An empty `list` results in a single boolean `false` being written.

**Complexity**:
- Time: O(N), where N is the number of entries.
- Space: O(1) auxiliary space.

**Determinism**: Deterministic.

---

### Mechanism 3: Success Response Serialization
**Intent**: Serialize the successful result of the `READDIRPLUS` NFS procedure.

**Inputs**:
- `dest`: A mutable reference to a type implementing `Write`.
- `arg`: A `Success` struct from `vfs::read_dir_plus`.

**Outputs**:
- Writes the XDR encoded `READDIRPLUS3resok` body to `dest`.

**Steps**:
1. Serialize the optional `dir_attr` (post-operation attributes of the directory) using `option` and `file_attr`.
2. Serialize the `cookie_verifier` as a fixed-size byte array using `array` (accessed via `.raw()`).
3. Serialize the list of directory entries using `dir_list_plus`.
4. Serialize the `eof` (End of File) flag as a boolean.

**Edge Cases**:
- If `dir_attr` is `None`, only the presence boolean is written.

**Complexity**:
- Time: O(N), where N is the number of entries in `arg.entries`.
- Space: O(1) auxiliary space.

**Determinism**: Deterministic.

---

### Mechanism 4: Failure Response Serialization
**Intent**: Serialize the failed result of the `READDIRPLUS` NFS procedure.

**Inputs**:
- `dest`: A mutable reference to a type implementing `Write`.
- `arg`: A `Fail` struct from `vfs::read_dir_plus`.

**Outputs**:
- Writes the XDR encoded `READDIRPLUS3resfail` body to `dest`.

**Steps**:
1. Serialize the optional `dir_attr` (attributes of the directory, which might be available even on failure) using `option` and `file_attr`.

**Edge Cases**:
- If `dir_attr` is `None`, only the presence boolean is written.

**Complexity**:
- Time: O(1).
- Space: O(1).

**Determinism**: Deterministic.

---

## 3. Dependency Mechanics

- **`crate::serializer::option`**: A generic mechanism used to serialize `Option<T>` types. It writes a boolean discriminator (`true` if `Some`, `false` if `None`) and conditionally calls a closure to serialize the inner value. This is critical for handling optional fields in NFS structures like `file_attr` and `dir_attr`.
- **`crate::serializer::array`**: A mechanism used to serialize fixed-size arrays (e.g., `[u8; N]`). This is used for the `cookie_verifier`.
- **`crate::serializer::files::file_attr`**: A mechanism that serializes the full set of file attributes (type, mode, nlink, uid, gid, size, etc.) into XDR format.
- **`crate::serializer::files::file_handle`**: A mechanism that serializes the opaque file handle variable-length array.
- **`crate::vfs::read_dir_plus::Entry`**: The data structure holding the logical components of a directory entry. The serializer consumes this struct, accessing fields like `file_id`, `cookie`, and optional attributes/handles.

---

## 4. Data Model

**Entities**:
- **`Entry`**: Represents a single item in a directory listing.
    - Fields: `file_id` (u64), `file_name` (String/Name), `cookie` (Cookie), `file_attr` (Option<Attr>), `file_handle` (Option<Handle>).
- **`Success`**: Represents the successful output of reading a directory.
    - Fields: `dir_attr` (Option<Attr>), `cookie_verifier` (CookieVerifier), `entries` (Vec<Entry>), `eof` (bool).
- **`Fail`**: Represents the failed output of reading a directory.
    - Fields: `dir_attr` (Option<Attr>).

**Relations**:
- `Success` contains 0..N `Entry` entities.
- `Entry` contains 0..1 `file::Attr` and 0..1 `file::Handle`.

**Global Invariants**:
- The `cookie_verifier` is treated as a fixed-size byte array in the serialization logic (via `array`), though the exact size is determined by the type definition of `CookieVerifier` in the VFS module.
- The list of entries is strictly terminated by a boolean `false` in the serialized stream.

---

## 5. Error Model

**Error Types**:
- `std::io::Error`: Represents a failure during the write operation (e.g., buffer full, network error).

**Error Propagation Strategy**:
- Propagation via the `?` operator. Any failure in the underlying `Write` trait or the helper serializers causes the current function to return immediately with the `io::Error`.

**Recoverability**:
- Not recoverable within this module. If an I/O error occurs, the serialization process is aborted, and the error is returned to the caller.

**Panics**:
- Allowed: No explicit panics are triggered by this module's logic.
- Conditions: Panics may occur if the underlying `Write` implementation panics or if the `cookie_verifier.raw()` call panics (though unlikely for standard types).

---

## 6. Traits

- **`std::io::Write`**: The `dest` parameter in all public functions is `&mut impl Write`. This module does not implement the trait but requires it as a bound for the output destination.

---

## 7. Overview

This module is used in order to convert high-level Virtual File System (VFS) results for the `READDIRPLUS` operation into the standardized External Data Representation (XDR) format required by the NFSv3 protocol. The system contains a layered architecture where the VFS layer handles file system logic and state management, while the serializer layer handles protocol-specific encoding.

The `READDIRPLUS` operation is distinct from standard directory reading because it returns not just file names and cookies, but also file attributes and file handles for each entry, allowing clients to avoid subsequent `LOOKUP` calls. This module is responsible for efficiently packing this data into a byte stream.

A typical usage scenario involves the server receiving a `READDIRPLUS` request, the VFS performing the directory scan and populating a `Success` struct with entries, and this module serializing that struct into a byte buffer to be sent back over the network to the NFS client.

Inside the system, the following things happen and they use:
1.  **Data Transformation**: The logical `Entry` objects are transformed into a stream of bytes. The `option` helper is used extensively to handle optional data (attributes/handles) efficiently, saving bandwidth when data is not available or requested.
2.  **Structure Encoding**: The `dir_list_plus` function manually constructs a linked list structure in the output stream by prefixing every entry with a `true` boolean and terminating the list with a `false` boolean, adhering to the XDR specification for variable-length lists of complex types.
3.  **Delegation**: The module delegates the serialization of complex sub-types (attributes, handles, names) to specialized modules (`serializer::files`), ensuring consistency across different NFS procedures.