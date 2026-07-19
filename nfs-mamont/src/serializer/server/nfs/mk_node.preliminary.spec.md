<!-- SPEC_HASH: b51b20fcadbf2cefa05d2c244013aca98c66209e2234ce901544a1cc23c2dbe9 -->
# Module Specification

Module: nfs_mamont::serializer::server::nfs::mk_node
Rust File: src/serializer/server/nfs/mk_node.rs

---

## 1. Dependencies

- **`std::io::Write`**: Used as the abstraction for the destination byte stream. The module writes XDR encoded bytes to any implementor of this trait (e.g., a TCP stream or a memory buffer).
- **`crate::serializer::files`**: Provides specific XDR serializers for file system entities.
  - `file_handle`: Encodes the opaque identifier for the created file.
  - `file_attr`: Encodes the attributes (mode, size, etc.) of the created file.
  - `wcc_data`: Encodes Weak Cache Consistency data, which is essential for the client to synchronize its cache regarding the directory where the node was created.
- **`crate::serializer::option`**: Provides the generic mechanism to serialize `Option` types according to XDR rules (a boolean discriminator followed by the value if present). This is used for the optional file handle and attributes in the success response.
- **`crate::vfs::mk_node`**: Provides the domain-level data structures representing the outcome of the `MKNOD` operation.
  - `Success`: Contains the data required for the `MKNOD3resok` XDR union.
  - `Fail`: Contains the data required for the `MKNOD3resfail` XDR union.

---

## 2. Mechanics

### Mechanism 1: Serialization of Successful MKNOD Result (`result_ok`)

**Intent:**
To convert a high-level representation of a successful `MKNOD` operation into the XDR format defined for `MKNOD3resok`. This allows the server to inform the client about the newly created special file's handle and attributes, as well as the state of the parent directory.

**Inputs:**
- `dest`: A mutable reference to a byte stream implementing `std::io::Write`.
- `arg`: A `mk_node::Success` struct containing:
  - `file`: An optional file handle for the new node.
  - `attr`: Optional attributes for the new node.
  - `wcc_data`: Weak cache consistency data for the parent directory.

**Outputs:**
- `io::Result<()>`: Indicates success or an I/O error during writing.

**Steps:**
1. Serialize the `file` field using the `option` helper. If present, it invokes `file_handle` to write the handle bytes.
2. Serialize the `attr` field using the `option` helper. If present, it invokes `file_attr` to write the attribute structure.
3. Serialize the `wcc_data` field by invoking `wcc_data`.

**Edge Cases:**
- If `arg.file` is `None`, the `option` serializer writes a zero (false) discriminator and skips the handle serialization.
- If `arg.attr` is `None`, the `option` serializer writes a zero discriminator and skips the attribute serialization.

**Complexity:**
- Time: O(N), where N is the total size of the file handle, attributes, and WCC data.
- Space: O(1) auxiliary space (excluding the output buffer).

**Determinism:**
- Deterministic. The same input struct produces the exact same byte sequence.

### Mechanism 2: Serialization of Failed MKNOD Result (`result_fail`)

**Intent:**
To convert a high-level representation of a failed `MKNOD` operation into the XDR format defined for `MKNOD3resfail`. Even on failure, the server must return Weak Cache Consistency data for the directory to allow the client to validate its cache.

**Inputs:**
- `dest`: A mutable reference to a byte stream implementing `std::io::Write`.
- `arg`: A `mk_node::Fail` struct containing:
  - `dir_wcc`: Weak cache consistency data for the parent directory.

**Outputs:**
- `io::Result<()>`: Indicates success or an I/O error during writing.

**Steps:**
1. Serialize the `dir_wcc` field by invoking `wcc_data`.

**Edge Cases:**
- None specific to the logic, though the `wcc_data` implementation handles optional pre- and post-operation attributes internally.

**Complexity:**
- Time: O(M), where M is the size of the WCC data.
- Space: O(1) auxiliary space.

**Determinism:**
- Deterministic.

---

## 3. Dependency Mechanics

- **`crate::serializer::option`**:
  - `option(dest, opt, cont)`: Serializes a boolean flag indicating presence. If true, calls the closure `cont` to serialize the inner value. This is critical for NFSv3 which uses optional fields extensively to save bandwidth.
- **`crate::serializer::files`**:
  - `file_handle(dest, fh)`: Serializes the opaque file handle bytes.
  - `file_attr(dest, attr)`: Serializes the standard NFS file attributes (type, mode, nlink, uid, gid, size, etc.).
  - `wcc_data(dest, wcc)`: Serializes the `wcc_data` structure, which encapsulates `pre_op_attr` (optional attributes before the operation) and `post_op_attr` (optional attributes after the operation).

---

## 4. Data Model

**Entities:**
- **`mk_node::Success`**: A data transfer object representing a successful node creation. It holds the resulting file handle, the resulting attributes, and the WCC data for the directory.
- **`mk_node::Fail`**: A data transfer object representing a failed node creation. It holds the WCC data for the directory. Note that the specific error code is not part of this struct's serialization logic here, as it is handled by the discriminated union wrapper in the calling context.

**Relations:**
- `Success` contains `vfs::WccData`.
- `Fail` contains `vfs::WccData`.

**Global Invariants:**
- The XDR format requires that optional fields are preceded by a boolean discriminator.
- The `result_fail` function specifically does *not* serialize the error code itself, as the error code determines which union arm (`resok` vs `resfail`) is selected at a higher level of the protocol stack.

---

## 5. Error Model

**Error Types:**
- `std::io::Error`: Represents errors during the writing process (e.g., buffer full, broken pipe).

**Error Propagation Strategy:**
- Propagation. The functions return `io::Result<()>`. Any error returned by the underlying `Write` trait or the helper serializers is immediately propagated up to the caller.

**Recoverability:**
- Not recoverable within this module. If a write fails, the serialization process aborts, and the error is returned to the RPC handler layer.

**Panics:**
- Allowed: No.
- Conditions: This module performs no explicit panics. It relies on safe Rust and the correctness of the `Write` implementation.

---

## 6. Traits

This module does not implement any traits. It uses the `std::io::Write` trait as a bound on the `dest` parameter.

---

## 7. Overview

This module is used in order to **serialize the results of the NFSv3 MKNOD procedure into the XDR wire format**.

The system contains a layered architecture where the **VFS (Virtual File System)** layer handles the logic of creating special files (character devices, block devices, named pipes, sockets) and returns Rust structs (`Success` or `Fail`). The **Serializer** layer is responsible for converting these internal Rust structs into the standardized byte stream defined by the NFSv3 protocol specification (RFC 1813).

A typical usage scenario of the system involves the server receiving an RPC request to create a special file. The VFS attempts to create the node. If successful, it returns a `Success` struct containing the new file's handle and attributes. The `result_ok` function in this module is then called to write this data into the response buffer. If the VFS fails (e.g., due to permissions or lack of space), it returns a `Fail` struct. The `result_fail` function is called to write the directory's cache consistency data to the response buffer.

Inside the system the following things happen and they use **XDR encoding rules**:
1. The `result_ok` function handles the `MKNOD3resok` structure. It serializes the `file` handle and `attr` as optional fields (using the `option` serializer) because the protocol allows the server to omit them if they are not supported or too expensive to retrieve. It always serializes the `wcc_data` to ensure the client can update its cache for the directory.
2. The `result_fail` function handles the `MKNOD3resfail` structure. It serializes only the `dir_wcc` data. This is crucial because even if the operation failed, the client needs to know if the directory's attributes changed (e.g., modification time) to maintain cache coherency.

This module isolates the specific byte-layout logic of the NFSv3 MKNOD response from the general file system logic, ensuring that protocol compliance is maintained locally within the serializer crate.