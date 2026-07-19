<!-- SPEC_HASH: b73f9aa9990bcc5a484a09ea262ceb341251177189ee8fe2c799d6a89bc9fb43 -->
# Module Specification

Module: nfs_mamont::serializer::server::nfs::read_dir
Rust File: src/serializer/server/nfs/read_dir.rs

---

## 1. Dependencies

- **std::io (Write)**
  - Purpose: Provides the trait for the destination buffer where the XDR encoded bytes are written. The module is agnostic to the specific writer (e.g., TCP stream, Vec<u8>), requiring only the ability to write bytes.

- **crate::serializer::files**
  - Purpose: Used to serialize complex file system attributes and names. Specifically, `file_attr` is used to encode directory attributes, and `file_name` is used to encode entry names within the directory listing.

- **crate::serializer**
  - Purpose: Provides primitive XDR serialization helpers. `u64` and `bool` are used for basic data types. `array` is used for the fixed-size `cookie_verifier`. `option` is used to handle optional fields like `dir_attr` according to XDR rules (encoding a boolean presence flag followed by the value).

- **crate::vfs::read_dir**
  - Purpose: Defines the high-level domain types resulting from a VFS `read_dir` operation. The module consumes `read_dir::Success` and `read_dir::Fail` structs, which contain the logical data (entries, attributes, cookies) that must be converted into the wire format.

---

## 2. Mechanics

### Mechanism: Entry Serialization
- **Intent**: To convert a single directory entry from the VFS representation into the XDR format required by the NFSv3 protocol.
- **Inputs**:
  - `dest`: A mutable reference to a writer implementing `std::io::Write`.
  - `entry`: An `Entry` struct containing `file_id`, `file_name`, and `cookie`.
- **Outputs**: `io::Result<()>` indicating success or failure of the write operations.
- **Steps**:
  1. Serialize the `file_id` as a 64-bit unsigned integer.
  2. Serialize the `file_name` using the specific file name serializer.
  3. Serialize the `cookie` as a 64-bit unsigned integer (extracted via `.raw()`).
- **Edge Cases**: None specific to the logic; relies on the underlying writer not failing.
- **Complexity**:
  - Time: O(1) relative to the number of entries (per entry).
  - Space: O(1) auxiliary space.
- **Determinism**: Deterministic (same input produces same byte sequence).

### Mechanism: Linked List Serialization
- **Intent**: To serialize a vector of entries into the XDR "linked list" structure defined by NFSv3. In XDR, a variable-length list is often represented as a sequence of [bool, value] terminated by a `false` bool.
- **Inputs**:
  - `dest`: A mutable reference to a writer.
  - `list`: A `Vec<Entry>` containing the directory entries to serialize.
- **Outputs**: `io::Result<()>`.
- **Steps**:
  1. Iterate over each entry in the vector.
  2. For each entry, write a boolean `true` to indicate the presence of a next entry.
  3. Serialize the entry using the `entry` mechanism.
  4. After the loop, write a boolean `false` to signify the end of the list.
- **Edge Cases**: An empty list results in a single `false` byte being written.
- **Complexity**:
  - Time: O(N) where N is the number of entries.
  - Space: O(1) auxiliary space.
- **Determinism**: Deterministic.

### Mechanism: Success Result Serialization
- **Intent**: To serialize the `READDIR3resok` structure, which is the successful response body for the NFSv3 READDIR procedure.
- **Inputs**:
  - `dest`: A mutable reference to a writer.
  - `arg`: A `read_dir::Success` struct containing `dir_attr`, `cookie_verifier`, `entries`, and `eof`.
- **Outputs**: `io::Result<()>`.
- **Steps**:
  1. Serialize the optional `dir_attr` using the `option` helper (which handles the presence flag and delegates to `file_attr`).
  2. Serialize the `cookie_verifier` as a fixed-size byte array.
  3. Serialize the list of entries using the `dir_list` mechanism.
  4. Serialize the `eof` flag as a boolean.
- **Edge Cases**: If `dir_attr` is `None`, the `option` helper writes a `false` boolean and skips the attribute data.
- **Complexity**:
  - Time: O(N) where N is the number of entries.
  - Space: O(1) auxiliary space.
- **Determinism**: Deterministic.

### Mechanism: Failure Result Serialization
- **Intent**: To serialize the `READDIR3resfail` structure, which is the failure response body for the NFSv3 READDIR procedure.
- **Inputs**:
  - `dest`: A mutable reference to a writer.
  - `arg`: A `read_dir::Fail` struct containing `dir_attr`.
- **Outputs**: `io::Result<()>`.
- **Steps**:
  1. Serialize the optional `dir_attr` using the `option` helper.
- **Edge Cases**: If `dir_attr` is `None`, only the presence flag (`false`) is written.
- **Complexity**:
  - Time: O(1).
  - Space: O(1).
- **Determinism**: Deterministic.

---

## 3. Dependency Mechanics

- **nfs_mamont::serializer::files::file_attr**
  - Mechanism: Serializes file attributes (mode, nlink, uid, gid, size, etc.) into XDR format.
  - Importance: Used to encode the post-operation attributes of the directory in both success and failure cases.

- **nfs_mamont::serializer::files::file_name**
  - Mechanism: Serializes a file name string into XDR format (length-prefixed).
  - Importance: Used to encode the name of each file in the directory entry list.

- **nfs_mamont::serializer::option**
  - Mechanism: Serializes an `Option` type. Writes a boolean `true` followed by the value if `Some`, or `false` if `None`.
  - Importance: Critical for handling optional fields like `dir_attr` in the NFS protocol.

- **nfs_mamont::serializer::array**
  - Mechanism: Serializes a fixed-size byte array (opaque data).
  - Importance: Used to serialize the `cookie_verifier`, which is a fixed-length byte array in the protocol.

- **nfs_mamont::vfs::read_dir::Entry**
  - Mechanism: Data structure holding `file_id`, `file_name`, and `cookie`.
  - Importance: The fundamental unit of data being serialized in the directory listing.

---

## 4. Data Model

**Entities:**
- **Entry**: Represents a single item in a directory listing.
  - `file_id`: Unique identifier for the file (u64).
  - `file_name`: Name of the file.
  - `cookie`: Opaque value used for subsequent READDIR requests to resume reading.
- **Success**: Represents a successful READDIR operation result.
  - `dir_attr`: Attributes of the directory (optional).
  - `cookie_verifier`: Verifier to ensure directory hasn't changed between calls.
  - `entries`: List of `Entry` objects.
  - `eof`: Boolean indicating if the end of the directory has been reached.
- **Fail**: Represents a failed READDIR operation result.
  - `dir_attr`: Attributes of the directory (optional).

**Relations:**
- `Success` contains 0..N `Entry` entities.

**Global Invariants:**
- The `dir_list` serialization must strictly follow the pattern `[bool(true), entry]*, bool(false)` to represent a valid XDR linked list.
- The `cookie_verifier` is a fixed-size array (`NFS3_COOKIEVERFSIZE`), defined by the dependency `vfs::read_dir`.

---

## 5. Error Model

**Error Types:**
- `std::io::Error`: Propagated from the underlying `Write` trait implementation (e.g., broken pipe, disk full).

**Error Propagation Strategy:**
- The module uses the `?` operator to propagate `io::Result` errors immediately upwards. It does not attempt to handle or transform errors.

**Recoverability:**
- Not recoverable within this module. If a write fails, the serialization process aborts, and the error is returned to the caller.

**Panics:**
- Allowed: No.
- Conditions: The code itself does not contain explicit panic points (like `unwrap()` or `expect()`). Panics would only originate from the underlying `Write` implementation or dependency functions.

---

## 6. Traits

- **std::io::Write**: Used by the public functions (`result_ok`, `result_fail`) via the `dest` parameter. The module does not implement the trait but relies on it as a bound (`&mut impl Write`).

---

## 7. Overview

This module is used in order to convert high-level Virtual File System (VFS) representations of directory listings into the standardized External Data Representation (XDR) format required by the NFSv3 protocol. It acts as a translation layer between the server's internal logic (which operates on Rust structs) and the network protocol (which operates on a byte stream).

This system contains the serialization logic for the server-side response of the `READDIR` procedure. It relies on a lower-level serializer framework (`crate::serializer`) for primitive types and specific file attribute serializers (`crate::serializer::files`) for complex types. The input data comes from the VFS layer (`crate::vfs::read_dir`), which abstracts the actual file system operations.

A typical usage scenario of the system involves the server handling a `READDIR` RPC request. The VFS layer retrieves directory entries and returns a `Success` struct containing file IDs, names, and cookies. This module then takes that struct and serializes it into a byte buffer (e.g., a TCP stream). The serialization process handles specific protocol details, such as encoding the entry list as a linked list of boolean flags and entries, and handling optional attributes.

Inside the system the following things happen and they use:
1.  **Entry Encoding**: The `entry` function converts individual file system metadata into XDR primitives using `u64` and `file_name` helpers.
2.  **List Construction**: The `dir_list` function iterates over the VFS entries, constructing the specific linked list structure required by the NFSv3 specification using boolean markers.
3.  **Response Assembly**: The public functions `result_ok` and `result_fail` assemble the final response structure, integrating attributes, verifiers, and the entry list into the final wire format.