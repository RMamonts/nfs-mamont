<!-- SPEC_HASH: ce7d1fe080b2b2d071f709a4efe8664a1ed2e6ec9ba797cc305dd2c7f6b0f3b1 -->
# Module Specification

Module: nfs_mamont::parser::read_buffer
Rust File: src/parser/read_buffer.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`std::io::{Read, ErrorKind}`**: Used to define the synchronous interface for the `CountBuffer` and `ReadBuffer` via the `Read` trait. `ErrorKind::UnexpectedEof` is specifically used to detect when a parsing operation runs out of buffered data, triggering the retry logic.
- **`tokio::io::{AsyncRead, AsyncReadExt}`**: Used to interact with the underlying network socket asynchronously. The `AsyncReadExt` trait provides methods like `read` and `read_exact` which are awaited to fill the internal buffers or read directly into destination buffers.
- **`crate::parser::{Error, Result}`**: Used to unify error handling. The `Result` type is the return type for parsing functions. The `Error::IO` variant is checked specifically for `UnexpectedEof` to determine if a parsing failure is due to a lack of data (recoverable) or a genuine error (unrecoverable).

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To bridge the gap between asynchronous network I/O (`AsyncRead`) and synchronous, stateful parsing logic (`Read`).
- To implement a retry mechanism that allows parsing to restart from the beginning of a message if the initial buffer did not contain enough data to complete the parse.
- To optimize bulk data transfer (e.g., file payloads) by allowing direct reads from the socket into the target memory, bypassing internal buffers.

Inputs:
- `socket`: An asynchronous stream implementing `AsyncRead + Unpin`.
- `capacity`: The size in bytes for the internal buffers.
- `caller`: A synchronous closure/function that attempts to parse a value from the `CountBuffer`.
- `dest`: A mutable byte slice to be filled with data.

Outputs:
- `Result`: The parsed value of type `T` or an `Error`.
- `usize`: The number of bytes read or discarded.
- `io::Result`: Standard I/O results for direct reading operations.

Steps:
1. **Double Buffering Initialization**: `CountBuffer` initializes two `ReadBuffer` instances. One is designated as the `read` buffer (source for parsing) and the other as the `write` buffer (target for socket reads).
2. **Parsing with Retry (`parse_with_retry`)**:
   - The method invokes the provided `caller` closure, passing `&mut self` (which implements `Read`).
   - If the closure returns `Err(Error::IO(err))` where `err.kind() == UnexpectedEof`, the system enters `retry_mode`.
   - In `retry_mode`, `fill_internal` is called asynchronously to read more data from the socket into the `write` buffer.
   - The read positions of both internal buffers are reset to their state at the start of the parsing attempt (`retry_start_read`, `retry_start_write`). This allows the parser to restart reading the message from the beginning.
   - The loop continues until parsing succeeds or a non-EOF error occurs.
3. **Buffer Swapping**: Upon successful parsing in `retry_mode`, the `read` buffer is cleared. The `read` and `write` indices are swapped (modulo 2). This makes the buffer that was previously receiving data (and likely contains the start of the next message) the new active read buffer.
4. **Direct Reading (`read_from_async`)**: When large payloads are expected, this method reads directly from the socket into the provided `dest` slice using `read_exact`. This bypasses the internal buffers to avoid unnecessary memory copies. The `total_bytes` counter is updated.
5. **Discarding (`discard_bytes`)**:
   - The method first consumes bytes available in the internal `read` and `write` buffers.
   - If more bytes need to be discarded than are buffered, it reads from the socket into the `write` buffer's free space (using it as a temporary scratchpad) and advances the write pointer, effectively discarding the data.
   - It validates that the exact number of requested bytes were discarded.

Edge Cases:
- **Simultaneous Read/Write**: The code explicitly checks if `self.read == self.write` after a successful parse in retry mode and returns an error if true, as this would imply reading and writing to the same buffer, breaking the double-buffering logic.
- **Empty Destination**: `read_from_inner` returns `Ok(0)` immediately if the destination buffer is empty.
- **Socket Closure**: `fill_internal` returns `UnexpectedEof` if the socket returns 0 bytes (connection closed), propagating a failure up the stack.

Complexity:
- Time: O(N) where N is the number of bytes processed. Parsing may involve multiple passes over the same data if retries occur (rewinding), but each byte is copied a bounded number of times.
- Space: O(C) where C is the fixed `capacity` of the buffers provided at construction.

Determinism:
- Deterministic. The behavior is strictly determined by the sequence of bytes provided by the socket and the logic of the parsing closure.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::parser`**:
 - **`Result` and `Error`**: The module relies on the `Result` type alias for its public interface. Crucially, it relies on the `Error` enum wrapping `std::io::Error`. The `parse_with_retry` mechanism specifically pattern matches on `Error::IO` to extract the `io::Error` and check its `ErrorKind`. This allows the module to distinguish between "need more data" (recoverable) and "corrupt stream" (fatal).

---

## 4. Data Model

Entities:
- **`CountBuffer<S: AsyncRead + Unpin>`**: The main buffered reader struct.
 - `bufs`: `Vec<ReadBuffer>` - A vector containing exactly two internal buffers.
 - `read`: `usize` - Index (0 or 1) of the buffer currently being used for parsing.
 - `write`: `usize` - Index (0 or 1) of the buffer currently being used to receive data from the socket.
 - `retry_mode`: `bool` - Flag indicating if the current operation is a retry due to insufficient data.
 - `socket`: `S` - The underlying asynchronous network stream.
 - `total_bytes`: `usize` - Counter for the total number of bytes consumed.
- **`ReadBuffer`**: Internal helper struct for managing a contiguous byte buffer.
 - `data`: `Vec<u8>` - The underlying byte storage.
 - `read_pos`: `usize` - Current cursor for reading data from the buffer.
 - `write_pos`: `usize` - Current cursor for writing data into the buffer.

Relations:
- **Composition**: `CountBuffer` owns two instances of `ReadBuffer`.
- **Association**: `CountBuffer` owns the `socket` `S`.

Global Invariants:
- **Buffer Indices**: `read` and `write` must always be different (i.e., `read != write`) to ensure the double-buffering invariant holds.
- **Position Ordering**: In `ReadBuffer`, `read_pos <= write_pos <= data.len()`.
- **Capacity**: The `data` vector in `ReadBuffer` is initialized with a fixed capacity and never resized.

## 5. Error Model

Error Types:
- **`std::io::Error`**:
 - `UnexpectedEof`: Used internally to signal that a parsing operation needs more data. It is caught by `parse_with_retry` to trigger the retry loop.
 - `InvalidData`: Returned by `discard_bytes` if the socket does not provide the expected number of bytes to discard.
 - `Other`: Returned if `read` and `write` indices become equal during retry logic.
- **`crate::parser::Error`**: Wraps `std::io::Error`. This is the public error type returned by `parse_with_retry`.

Error Propagation Strategy:
- **Conversion**: `std::io::Error` is wrapped into `crate::parser::Error::IO` when returned from public methods.
- **Interception**: `parse_with_retry` intercepts `Error::IO` with `ErrorKind::UnexpectedEof` to implement retry logic. All other errors are propagated immediately to the caller.

Recoverability:
- **UnexpectedEof**: Recoverable. The module automatically attempts to read more data and retry the operation.
- **Connection Closed**: Unrecoverable. If the socket returns 0 bytes in `fill_internal`, an error is returned.
- **Invalid Data**: Unrecoverable. If `discard_bytes` fails to read the required amount, it returns an error.

Panics:
- Allowed: No explicit panics are triggered by the module logic.

---

## 6. Traits

List which external traits this module implements:
- **`std::io::Read`**: Implemented for `CountBuffer<S>` and `ReadBuffer`.
 - For `CountBuffer`, it reads sequentially from the `read` buffer and then the `write` buffer.
 - For `ReadBuffer`, it copies data from `data[read_pos..write_pos]` into the destination slice and advances `read_pos`.

---

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependencies. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to **manage the stream of incoming bytes for the RPC parser**, solving the impedance mismatch between the asynchronous, packet-based nature of network sockets and the synchronous, continuous requirements of XDR parsing logic.

The system contains a parser architecture where high-level parsing functions (defined in `nfs_mamont::parser` modules) expect a standard synchronous `Read` stream to decode XDR data structures. However, the data source is a Tokio TCP socket, which provides data asynchronously and in chunks that may not align with message boundaries. This module (`read_buffer`) acts as the adapter. It implements `Read` for `CountBuffer`, allowing the parser to consume bytes synchronously. Internally, it manages the complexity of fetching those bytes asynchronously from the socket.

A typical usage scenario of the system involves the server receiving a fragmented NFS request. The RPC dispatcher creates a `CountBuffer` wrapping the client socket. It calls `parse_with_retry`, passing a closure that contains the logic to parse the specific RPC procedure arguments. The closure reads from the `CountBuffer`. If the closure hits the end of the available data (returns `UnexpectedEof`), `CountBuffer` suspends the synchronous parsing, asynchronously waits for more packets from the network, fills its secondary buffer, and then *rewinds* the parsing state to allow the closure to run again from the start of the message. This process repeats until the full message is parsed.

Inside the system, the following things happen and they use this module:
- **Zero-Copy Optimization**: For operations like `WRITE` that carry large data payloads, the parser uses `read_from_async` to read the payload directly from the socket into the final memory buffer, bypassing the internal `CountBuffer` storage to minimize CPU usage and memory bandwidth.
- **Message Boundary Handling**: The double-buffering strategy allows the system to keep the "tail" of the previous message (which might contain the start of the next message due to TCP coalescing) in one buffer while filling the other buffer with new data, ensuring that no bytes are lost or misinterpreted between message parsing operations.
- **Flow Control**: By tracking `total_bytes`, the module allows the server to monitor bandwidth usage or enforce limits per connection.