<!-- SPEC_HASH: eae3070d711b3f5548df3b7b85b9f46bde00efdf7fbf115d669d66d241e41271 -->
# Module Specification

Module: nfs_mamont::allocator::slice
Rust File: src/allocator/slice.rs

---

## 1. Dependencies

From the provided context and dependency facts:

- **`std::sync::Arc`**: Used to share ownership of the `AllocatorState` between the `Slice` and the allocator. This allows the `Slice` to return memory to the pool independently of the allocator's lifecycle.
- **`std::ops::Range`**: Used to define the logical byte window (`start..end`) that the `Slice` exposes over the underlying physical buffers.
- **`super::Buffer`**: The trait implemented by `Slice`. This allows `Slice` to act as a generic handle to memory within the `nfs_mamont` allocator system, providing methods like `chunks` and `len`.
- **`super::UnownedBuffer`**: The type of the physical memory blocks stored within the `Slice`. The `Slice` aggregates these non-contiguous blocks into a contiguous logical view.
- **`super::AllocatorState`**: The resource management context containing the memory pool and semaphore. The `Slice` uses this to deallocate its buffers upon destruction.

---

## 2. Mechanics

**Intent:**
The module provides a logical, contiguous view over a collection of disjoint physical memory blocks (`UnownedBuffer`). It allows users to access a specific byte range within a list of buffers as if it were a single continuous array. Additionally, it manages the lifecycle of these physical blocks by automatically returning them to the allocator's pool and restoring semaphore permits when the `Slice` is dropped.

**Inputs:**
- **Construction (`Slice::new`)**:
  - `buffers`: A vector of `UnownedBuffer` instances representing the physical memory.
  - `range`: A `std::ops::Range<usize>` defining the start and end byte offsets accessible within the concatenated buffers.
  - `state`: An optional `Arc<AllocatorState>` used for returning buffers to the pool.
- **Iteration**: References to `Slice` (`&Slice` or `&mut Slice`).

**Outputs:**
- **Slice Instance**: A struct holding the buffers, the accessible range, and the allocator state.
- **Iterators**: `Iter` (yielding `&[u8]`) or `IterMut` (yielding `&mut [u8]`) that traverse the logical view.
- **Deallocation**: Side effect of pushing buffers back to `AllocatorState.pool` and adding permits to `AllocatorState.semaphore`.

**Steps:**

1.  **Construction (`Slice::new`)**:
    - Validates that `range.start <= range.end`.
    - Calculates the total length of all provided buffers.
    - Asserts that `range.start` and `range.end` are within the total buffer length.
    - Stores the buffers, range, and state.

2.  **Iteration (`Iter` / `IterMut`)**:
    - The iterator maintains a sliding window (`range`) over the sequence of buffers.
    - For each buffer, it calculates the intersection between the buffer's data and the current range window.
    - It updates the window by subtracting the current buffer's length from the window's start and end (using `saturating_sub`).
    - If the buffer's length is greater than the window's start, it returns a slice of the buffer corresponding to the window's bounds.
    - If the window's start equals its end, the iterator terminates.

3.  **Deallocation (`Slice::drop`)**:
    - If `state` is `Some`, the `Slice` drains its internal vector of `UnownedBuffer`s.
    - Each buffer is pushed back into `state.pool`.
    - The total count of returned buffers is added back to `state.semaphore` as permits.

**Edge Cases:**
- **Empty Slice**: `Slice::empty` creates a slice with no buffers and a 0-length range.
- **Zero-Size Range**: Iterators over a slice with `range.start == range.end` yield no items.
- **Mid-Buffer Bounds**: The range can start and end in the middle of physical buffers; the iterators correctly calculate the sub-slices.
- **Missing State**: If `state` is `None` during `drop`, no deallocation occurs (buffers are simply dropped).

**Complexity:**
- **Time**:
  - `new`: O(N) where N is the number of buffers (to sum lengths).
  - `next` (Iterator): Amortized O(1) per chunk yielded.
  - `drop`: O(N) to drain the vector and push to the pool.
- **Space**: O(N) to store the vector of buffers.

**Determinism:**
- Deterministic. The behavior is strictly defined by the input range and buffer lengths.

---

## 3. Dependency Mechanics

- **`AllocatorState` (from `nfs_mamont::allocator`)**:
  - **Resource Reclamation**: The `Slice` depends on the `pool` (`ArrayQueue`) and `semaphore` fields of `AllocatorState`. In its `Drop` implementation, it calls `pool.push` to return memory and `semaphore.add_permits` to signal availability. This creates a mechanism where the `Slice` acts as a smart pointer that automatically releases resources.

- **`UnownedBuffer` (from `nfs_mamont::allocator::buffer`)**:
  - **Storage Unit**: The `Slice` stores a `Vec<UnownedBuffer>`. It relies on `UnownedBuffer`'s ability to represent a mutable memory region without ownership, allowing the `Slice` to aggregate them and pass them back to the allocator later.

- **`Buffer` Trait (from `nfs_mamont::allocator`)**:
  - **Interface Compliance**: `Slice` implements `Buffer` to integrate with the rest of the system. This requires providing `chunks` and `chunks_mut` iterators, which `Slice` implements via its `IntoIterator` implementations.

---

## 4. Data Model

**Entities:**
- **`Slice`**: A logical view over a collection of buffers. Contains `buffers` (physical memory), `range` (logical bounds), and `state` (resource manager).
- **`Iter<'a>`**: A shared iterator over a `Slice`, yielding immutable byte slices.
- **`IterMut<'a>`**: A mutable iterator over a `Slice`, yielding mutable byte slices.

**Relations:**
- **`Slice` → `UnownedBuffer` (1:N)**: A `Slice` owns a vector of `UnownedBuffer` instances.
- **`Slice` → `AllocatorState` (0..1)**: A `Slice` holds an optional reference-counted pointer to the `AllocatorState`.
- **`Iter` → `Slice` (1:1)**: An iterator borrows a `Slice` for its lifetime.

**Global Invariants:**
- **Range Validity**: `range.start <= range.end` and `range.end <= sum(buffers.len())` must hold upon construction (enforced by assertions).
- **Buffer Integrity**: All `UnownedBuffer`s in `buffers` must point to valid memory for the duration of the `Slice`'s existence.

---

## 5. Error Model

**Error Types:**
- None. The module does not define custom error types.

**Error Propagation Strategy:**
- **Panic**: Used for invariant violations during construction.

**Recoverability:**
- **Construction**: Not recoverable. If `Slice::new` panics due to invalid range or buffer length, the process or task aborts.

**Panics:**
- **Allowed**: Yes.
- **Conditions**:
  - In `Slice::new`: If `range.start > range.end`.
  - In `Slice::new`: If `range.start` or `range.end` exceeds the total length of the provided buffers.
  - In `Slice::new`: If any buffer in the list is empty (`is_empty()` returns true).

---

## 6. Traits

The module implements the following traits:

- **`Buffer`** (from `super`): Implemented for `Slice`. Provides `chunks`, `chunks_mut`, `len`, `is_empty`, and `empty`.
- **`IntoIterator`**: Implemented for `&Slice` (yields `Iter`) and `&mut Slice` (yields `IterMut`).
- **`Iterator`**: Implemented for `Iter` (Item = `&[u8]`) and `IterMut` (Item = `&mut [u8]`).
- **`Drop`**: Implemented for `Slice` to handle automatic deallocation.
- **`PartialEq<[u8]>`** (cfg(test)): Implemented for `Slice` to compare its contents with a byte array.

---

## 7. Overview

This module is used in order to **abstract the physical fragmentation of memory into a contiguous logical interface** and **automate the reclamation of memory resources**. In the context of the `nfs_mamont` allocator, the system allocates fixed-size blocks (`UnownedBuffer`), but user operations often require variable-sized data streams. This module bridges that gap.

This system contains a **logical buffer abstraction** (`Slice`) that wraps a list of physical blocks. It allows the allocator to satisfy a request for $N$ bytes by aggregating multiple smaller blocks, presenting them to the user as a single unit via the `Buffer` trait. Crucially, it embeds the logic for returning these blocks to the pool, ensuring that the allocator's semaphore and pool remain consistent without requiring explicit user intervention.

A typical usage scenario of the system involves a network read operation. The allocator provides a `Slice` containing, for example, three 4KB blocks to hold a 10KB message. The user iterates over the `Slice` (via `chunks`), processing the data as a continuous stream of bytes. Once the message is processed, the `Slice` goes out of scope. The `Drop` trait then triggers, pushing the three 4KB blocks back into the allocator's queue and releasing the semaphore permits, making the memory immediately available for new connections.

Inside the system, the following things happen and they use this module:
1.  **Aggregation**: The allocator constructs a `Slice` from a `Vec<UnownedBuffer>`, defining a `range` that covers the requested data size.
2.  **Access**: The user code calls `chunks()` or `chunks_mut()`. The `Slice` creates an iterator that calculates the correct offsets within the underlying `UnownedBuffer`s, hiding the fact that the data is split across multiple memory addresses.
3.  **Reclamation**: When the `Slice` is dropped, it accesses the `AllocatorState` (via `Arc`) to execute `deallocate`. This restores the resources to the pool, closing the lifecycle loop of the memory allocation.

The critical aspect of this module is the **decoupling of the logical data view from the physical memory layout**, combined with **RAII-based resource management** to ensure safety and efficiency in a high-performance asynchronous environment.