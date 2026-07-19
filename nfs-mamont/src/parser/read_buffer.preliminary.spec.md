<!-- SPEC_HASH: ce7d1fe080b2b2d071f709a4efe8664a1ed2e6ec9ba797cc305dd2c7f6b0f3b1 -->
# Module Specification

Module: nfs_mamont::parser::read_buffer
Rust File: src/parser/read_buffer.rs

---

## 1. Dependencies

From the code analysis, the following external crates and standard library modules are used:

- **`std::io`**: Provides the `Read` trait and `ErrorKind` enum. The `Read` trait is implemented by `CountBuffer` and `ReadBuffer` to allow synchronous parsing functions to consume data. `ErrorKind::UnexpectedEof` is used specifically to detect when a parsing operation needs more data.
- **`tokio::io`**: Provides the `AsyncRead` trait and `AsyncReadExt` extension trait. These are used to define the source stream `S` and to perform asynchronous reads (`socket.read`) from the network.
- **`crate::parser`**: Provides the `Error` and `Result` types. The module uses these to propagate parsing failures, specifically wrapping I/O errors encountered during the async read phase into the parser's error domain.
- **`crate::rpc`**: (Inferred dependency via `crate::parser`) Defines the underlying `Error` enum structure. The `read_buffer` module relies on the existence of an `IO` variant within the parser's error type to wrap `std::io::Error`.

*Assumption:* It is assumed that `crate::parser::Error` is compatible with or wraps `std::io::Error` and exposes a mechanism (like `Error::IO`) to distinguish I/O failures, specifically `UnexpectedEof`, from other parsing errors.

---

## 2. Mechanics

This module implements a double-buffered asynchronous reader that bridges asynchronous network I/O with synchronous parsing logic, specifically designed for XDR-encoded streams where parsing may require backtracking or retrying upon partial data.

### Mechanism 1: Double-Buffered Parsing with Retry

Intent:
- To allow a synchronous parser (expecting a `Read` interface) to operate on an asynchronous stream (`AsyncRead`).
- To handle cases where the parser runs out of data mid-message by transparently fetching more data and resetting the parser's state to retry, without losing previously read data.

Inputs:
- `socket`: An asynchronous stream implementing `AsyncRead` and `Unpin`.
- `capacity`: The size of the internal buffers.
- `caller`: A synchronous closure/function that attempts to parse a value `T` from the buffer.

Outputs:
- `Result<T>`: The parsed value or an error.

Steps:
1. **Initialization**: Two `ReadBuffer` instances are created. One acts as the active read buffer, the other as the write buffer.
2. **Parsing Attempt**: The `parse_with_retry` method invokes the `caller` closure, passing `&mut self` (which implements `Read`).
3. **EOF Detection**: If the closure returns `Err(Error::IO(err))` where `err.kind() == UnexpectedEof`, the system enters retry mode.
4. **Refill**: The `fill_internal` method is called asynchronously to read data from the `socket` into the current *write* buffer.
5. **Reset**: The read positions of both internal buffers are reset to the values recorded at the start of the parsing attempt. This allows the `caller` to restart reading from the beginning of the logical message, now that more data is available.
6. **Loop**: The process repeats from step 2 until the parser succeeds or a non-EOF error occurs.
7. **Swap**: Upon success, if retry mode was active, the buffers are logically swapped (indices are incremented modulo 2). The old read buffer is cleaned, and the old write buffer (now containing the remaining unconsumed data) becomes the new read buffer.

Edge Cases:
- If the write buffer is full (`available_write() == 0`), `fill_internal` returns `Ok(0)`, effectively stalling the retry loop until space is freed (which implies a logic error in the parser or buffer sizing).
- If `read` and `write` indices point to the same buffer after a successful parse, an error is returned ("Cannot read and write to one buffer simultaneously").

Complexity:
- Time: Amortized O(N) for reading N bytes, as data is copied from the socket to the buffer and then to the parser.
- Space: O(2 * capacity), fixed size determined at creation.

Determinism:
- Deterministic (Given the same input stream and parser logic, the output is consistent).

### Mechanism 2: Byte Counting and Discarding

Intent:
- To track the total number of bytes consumed by the parser (for protocol requirements like record marking).
- To efficiently skip over specific byte ranges in the stream without buffering them unnecessarily if they exceed current buffer capacity.

Inputs:
- `n`: Number of bytes to discard.

Outputs:
- `io::Result<()>`: Success or error if the stream ends prematurely.

Steps:
1. **Buffer Consumption**: `discard_bytes` first consumes available data from the internal read buffers (`bufs[read]` and `bufs[write]`).
2. **Direct Discard**: If more bytes are needed, it reads directly from the `socket` into the write buffer's slice but immediately advances the write pointer without advancing the read pointer. This effectively overwrites the buffer or fills it with garbage data that will be ignored, serving to drain the socket.

---

## 3. Dependency Mechanics

Since specifications for `crate::parser` were not fully provided, the following mechanics are inferred based on the code and `*.facts.json`:

- **`crate::parser::Error`**: This is the primary error type. The `read_buffer` module specifically checks for `Error::IO` wrapping a `std::io::Error` with `ErrorKind::UnexpectedEof`. This implies the parser module defines an error type that aggregates I/O errors.
- **`tokio::io::AsyncRead::read`**: The fundamental async operation used to fetch data from the network. The module relies on this returning `Ok(0)` to signal connection closure (which is treated as an error) or `Ok(n)` to signal data received.

---

## 4. Data Model

Entities:
- **`CountBuffer<S>`**: The public interface. Wraps a socket `S` and manages two `ReadBuffer` instances. It tracks `read` and `write` indices (0 or 1) to identify which buffer is currently being used for parsing and which is being filled.
- **`ReadBuffer`**: A fixed-size circular buffer (conceptually, though implemented as a linear vector with moving cursors). Contains `data` (Vec), `read_pos`, and `write_pos`.

Relations:
- `CountBuffer` owns 2 `ReadBuffer`s.
- `CountBuffer` owns 1 `S` (AsyncRead stream).

Global Invariants:
- `read_pos <= write_pos` within a `ReadBuffer`.
- `read != write` indices in `CountBuffer` after a successful parse operation (ensures reading and writing happen in separate buffers).
- `total_bytes` reflects the number of bytes successfully consumed by the parser since the last `clean()` call.

---

## 5. Error Model

Error Types:
- **`crate::parser::Error`**: The main error type returned by public methods.
- **`std::io::Error`**: Wrapped inside `crate::parser::Error`. Specific kinds checked:
  - `UnexpectedEof`: Treated as a transient signal to fetch more data and retry.
  - Others: Treated as fatal errors.

Error Propagation Strategy:
- Custom enum (`crate::parser::Error`). The module converts `std::io::Error` from the socket into this type.

Recoverability:
- `UnexpectedEof` is recoverable via the `parse_with_retry` loop.
- Other I/O errors (e.g., `ConnectionReset`) are fatal and returned immediately to the caller.

Panics:
- Allowed: No explicit panics in the provided code. Logic errors (like buffer indices colliding) return `Err`.

---

## 6. Traits

The module implements the following external traits:

- **`std::io::Read` for `CountBuffer<S>`**: Allows the buffer to act as a synchronous input source for parsers. It reads from the internal buffers sequentially.
- **`std::io::Read` for `ReadBuffer`**: Internal implementation to copy data from the internal vector to an external destination slice.

---

## 7. Overview

This module is used in order to bridge the gap between the asynchronous nature of network I/O (Tokio) and the synchronous, stateful nature of XDR/RPC parsing. It solves the specific problem of parsing variable-length messages from a TCP stream where a parser might attempt to read more bytes than are currently available in the kernel buffer.

This system contains a buffering mechanism (`CountBuffer`) that allows a parser to "peek" at the stream, fail if data is missing, and then transparently resume once more data arrives, without the parser needing to implement complex async state machines itself. It also handles the accounting of bytes consumed, which is critical for protocols like NFS that use Record Marking (RFC 5531) to delineate messages.

A typical usage scenario of the system involves a network task receiving a TCP stream. It wraps this stream in a `CountBuffer`. It then calls `parse_with_retry`, passing a closure that contains the logic to parse an RPC header and arguments. If the closure returns `UnexpectedEof` because the message body hasn't fully arrived yet, `CountBuffer` asynchronously waits for more data, resets the parser's cursor, and calls the closure again.

Inside the system, the following things happen and they use:
- **Double Buffering**: Uses two `ReadBuffer` instances to allow reading (parsing) from one buffer while writing (receiving from socket) into the other, or to preserve the state of the current message while fetching the next chunk.
- **Retry Logic**: Uses `parse_with_retry` to catch `UnexpectedEof`, trigger `fill_internal` (async read), and reset buffer positions via `reset_read`.
- **Stream Management**: Uses `discard_bytes` to skip over padding or unused data in the stream, updating `total_bytes` to maintain synchronization with the protocol's framing.