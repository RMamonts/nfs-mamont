<!-- SPEC_HASH: eae3070d711b3f5548df3b7b85b9f46bde00efdf7fbf115d669d66d241e41271 -->
# Module Specification

Module: nfs_mamont::allocator::slice
Rust File: src/allocator/slice.rs

---

## 1. Dependencies

From the provided context and dependency facts:

- **`std::sync::Arc`**: Used to hold a reference-counted pointer to the `AllocatorState`. This allows the `Slice` to return the underlying buffers to the allocator's pool and restore semaphore permits when the `Slice` is dropped, regardless of where it is in the codebase.
- **`std::ops::Range`**: Defines the byte-level bounds (`start` and `end`) of the logical view over the list of physical buffers.
- **`super::Buffer`**: The trait defining the interface for readable and writable chunked memory. `Slice` implements this trait to integrate with the rest of the allocator system.
- **`super::UnownedBuffer`**: The primitive type representing a raw mutable memory region without ownership semantics. `Slice` manages a collection of these.
- **`super::AllocatorState`**: A structure (assumed based on facts) containing a `pool` (an `ArrayQueue`) and a `semaphore`. The `Slice` interacts with these fields to deallocate memory.

---

## 2. Mechanics

**Intent:**
The module provides a logical, contiguous byte view over a fragmented list of physical memory buffers (`UnownedBuffer`). It allows treating multiple disjoint memory blocks as a single readable/writable entity defined by a specific byte range. Additionally, it manages the lifecycle of these buffers by automatically returning them to the central allocator pool upon destruction.

**Inputs:**
- A vector of `UnownedBuffer` instances representing the physical memory.
- A `Range<usize>` specifying the start and end byte offsets of the logical slice within the concatenated buffers.
- An optional `Arc<AllocatorState>` for resource management.

**Outputs:**
- A `Slice` instance acting as a window into the buffers.
- Iterators (`Iter`, `IterMut`) yielding byte slices (`&[u8]`, `&mut [u8]`) corresponding to the logical view.

**Steps:**
1. **Construction (`new`)**:
   - Validates that the range start is not greater than the end.
   - Calculates the total length of all provided buffers.
   - Asserts that the range bounds fall within the total length.
   - Stores the buffers, the range, and the allocator state.
2. **Iteration (`Iter`, `IterMut`)**:
   - The iterator maintains a mutable copy of the byte range.
   - It traverses the underlying `Vec<UnownedBuffer>`.
   - For each buffer, it checks if the buffer overlaps with the current range.
   - If the buffer is completely before the range start, it subtracts the buffer's length from the range start and end (effectively skipping it) and continues.
   - If the buffer overlaps the range, it calculates the intersection: `buffer[start..end.min(buffer_len)]`.
   - It returns this slice and updates the range start/end by subtracting the current buffer's length, effectively moving the window forward.
3. **Deallocation (`Drop`)**:
   - When the `Slice` goes out of scope, the `deallocate` method is called.
   - If an `AllocatorState` is present, it drains all `UnownedBuffer`s from the internal vector.
   - Each buffer is pushed back into the `state.pool`.
   - Permits corresponding to the number of buffers returned are added to `state.semaphore`.

**Edge Cases:**
- **Empty Slice**: Can be created via `empty()` or if the range is `0..0`. Iteration yields no items.
- **Range Alignment**: The range can start and end in the middle of buffers. The iterator logic handles slicing the first and last buffers appropriately.
- **Zero-length Buffers**: The constructor asserts that no buffer in the input vector is empty.

**Complexity:**
- **Time**:
  - `new`: O(N) where N is the number of buffers (to sum lengths).
  - `next` (iterator): Amortized O(1) per chunk.
  - `drop`: O(N) where N is the number of buffers (to push back to pool).
- **Space**: O(N) for storing the vector of buffers.

**Determinism:**
- Deterministic. The behavior is strictly defined by the input range and the order of buffers in the vector.

---

## 3. Dependency Mechanics

- **`UnownedBuffer` (from `nfs_mamont::allocator::buffer`)**:
  - **Deref/DerefMut**: The `Slice` relies on these implementations to treat `UnownedBuffer` instances as standard `&[u8]` or `&mut [u8]` slices within the iterators.
  - **Raw Memory Handling**: The `Slice` assumes the `UnownedBuffer` is valid for the lifetime of the `Slice` and does not attempt to free the memory itself, delegating that to the `AllocatorState`.

- **`AllocatorState` (from `nfs_mamont::allocator`)**:
  - **Pool Management**: The `Slice` uses the `pool` field (assumed `ArrayQueue`) to return buffers. This implies the allocator uses a fixed-size ring buffer or similar structure for recycling memory.
  - **Semaphore Signaling**: The `Slice` uses the `semaphore` field to increment permits, signaling to waiting tasks (likely in an async context) that memory has become available.

- **`Buffer` Trait (from `nfs_mamont::allocator`)**:
  - **Interface Compliance**: The `Slice` implements `chunks` and `chunks_mut` to satisfy this trait, allowing it to be used polymorphically wherever a generic buffer is required.

---

## 4. Data Model

**Entities:**
- **`Slice`**: The main container holding a list of buffers, a logical byte range, and a handle to the allocator state.
- **`Iter<'a>`**: A shared iterator yielding `&'a [u8]`.
- **`IterMut<'a>`**: A mutable iterator yielding `&'a mut [u8]`.

**Relations:**
- **`Slice` → `Vec<UnownedBuffer>` (1:N)**: The `Slice` owns a list of physical memory blocks.
- **`Slice` → `AllocatorState` (0..1)**: The `Slice` holds an optional reference-counted pointer to the allocator's state to facilitate resource return.

**Global Invariants:**
- **Range Validity**: `range.start <= range.end` must always hold.
- **Buffer Bounds**: The `range` must always be within the cumulative length of the `buffers` vector.
- **Non-empty Buffers**: No `UnownedBuffer` within the `buffers` vector can have a length of 0.

---

## 5. Error Model

**Error Types:**
- None. The module does not define custom error types.

**Error Propagation Strategy:**
- N/A.

**Recoverability:**
- N/A.

**Panics:**
- **Allowed**: Yes.
- **Conditions**:
  - In `Slice::new`: Panics if `range.start > range.end`.
  - In `Slice::new`: Panics if any buffer in the list is empty (`buffer.is_empty()`).
  - In `Slice::new`: Panics if `range.start` or `range.end` exceeds the total length of the buffers.

---

## 6. Traits

The module implements the following external traits:
- **`IntoIterator`** (for `&Slice` and `&mut Slice`): Converts references to the slice into the respective iterator types.
- **`Drop`** for `Slice`: Handles the automatic deallocation of buffers back to the pool.
- **`Buffer`** (from `super`): Implements the interface for chunked memory access (`chunks`, `chunks_mut`, `len`, `is_empty`, `empty`).
- **`PartialEq<[u8]>`** (test only): Allows comparing the content of the slice with a byte array.

---

## 7. Overview

This module is used in order to provide a **contiguous logical abstraction over fragmented physical memory** while managing the **lifecycle of that memory** via a custom allocator.

In the context of the `nfs_mamont` allocator system, memory is not always allocated as a single contiguous block (e.g., due to fragmentation or specific allocation strategies). Instead, it may be represented as a list of `UnownedBuffer` objects. The `Slice` module bridges this gap: it takes a list of these disjoint buffers and a byte range, presenting them as a single, iterable sequence of bytes that conforms to the `Buffer` trait.

This system contains a mechanism for **zero-copy or low-copy data handling** where data spans multiple memory blocks. A typical usage scenario involves an allocator handing out a `Slice` to a consumer. The consumer reads or writes data using the `chunks` or `chunks_mut` methods, unaware that the data might be spread across several physical buffers.

Inside the system, the following things happen and they use this module:
1.  **Allocation**: The allocator constructs a `Slice` from one or more `UnownedBuffer`s retrieved from its internal pool.
2.  **Usage**: The consumer interacts with the `Slice` via the `Buffer` trait interface. The `Iter` and `IterMut` logic handles the complex arithmetic of mapping the logical byte range to the specific offsets within the underlying physical buffers.
3.  **Deallocation**: Once the `Slice` is no longer needed (dropped), it automatically returns the `UnownedBuffer`s to the `AllocatorState`'s pool and signals the semaphore. This ensures that memory is recycled efficiently and that flow control (via the semaphore) is maintained without manual intervention from the consumer.

The critical aspect of `Slice` is that it decouples the **logical view** of data (a continuous stream of bytes) from the **physical reality** (a vector of separate memory blocks), while simultaneously enforcing resource cleanup through RAII (Resource Acquisition Is Initialization).