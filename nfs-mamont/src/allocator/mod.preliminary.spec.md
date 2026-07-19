<!-- SPEC_HASH: 4590b0f9584d14ca1b6ccfabef16bcad0480b0beb038fccb7219e7512902130a -->
# Module Specification

Module: nfs_mamont::allocator
Rust File: src/allocator/mod.rs

---

## 1. Dependencies

From the provided context and dependency facts:

- **`std::alloc`**: Used for low-level memory management. Specifically, `alloc::alloc_zeroed` is used to reserve the entire memory block for the allocator at initialization, and `alloc::dealloc` is used to release it when the allocator is dropped.
- **`crossbeam_queue::ArrayQueue`**: Used to implement the buffer pool (`AllocatorState.pool`). It provides a lock-free, bounded queue suitable for concurrent access to the pre-allocated `UnownedBuffer` instances.
- **`tokio::sync::Semaphore`**: Used to enforce backpressure and limit concurrency. It tracks the number of available buffers, allowing asynchronous tasks to wait (`acquire_many`) until resources are free instead of busy-spinning or failing immediately.
- **`libc`** (feature `mlock`): Used to call `mlock` and `munlock`. This ensures that the allocated memory pages are kept in physical RAM and not swapped to disk, which is critical for consistent performance in high-throughput networking (NFS) scenarios.
- **`nfs_mamont::allocator::buffer`**: Provides the `UnownedBuffer` type, which serves as the atomic unit of memory in the pool. It allows the allocator to distribute raw memory slices without enforcing complex lifetime constraints on the allocator itself.
- **`nfs_mamont::allocator::slice`**: Provides the `Slice` type, which implements the `Buffer` trait. It is used to aggregate multiple `UnownedBuffer` instances into a single logical view for the user and handles the automatic return of memory to the pool upon destruction.

---

## 2. Mechanics

**Intent:**
The module implements a bounded, asynchronous memory allocator designed for high-performance data transmission. It pre-allocates a fixed-size memory arena (a "pool") at startup to eliminate runtime allocation latency and fragmentation. It provides an asynchronous interface (`Allocator`) that aggregates smaller fixed-size blocks into variable-sized logical buffers (`Slice`), applying backpressure via a semaphore when the pool is exhausted.

**Inputs:**
- **Configuration (`Impl::new`)**: `size` (bytes per block), `count` (number of blocks).
- **Allocation Request (`Impl::allocate`)**: `size` (minimum bytes required).

**Outputs:**
- **Allocator Instance**: A handle to the memory pool.
- **Buffer (`Slice`)**: A logical view over one or more physical memory blocks, or `None` if the request exceeds total capacity.

**Steps:**

1.  **Initialization (`Impl::new`)**:
    - Calculates the total memory required (`size * count`).
    - Allocates a single contiguous block of zeroed memory using the global allocator.
    - Optionally locks the memory using `mlock` to prevent swapping.
    - Splits the contiguous block into `count` distinct `UnownedBuffer` instances.
    - Pushes all `UnownedBuffer` instances into an `ArrayQueue` (the pool).
    - Initializes a `Semaphore` with `count` permits.

2.  **Allocation (`Impl::allocate`)**:
    - Checks if the requested `size` exceeds the total capacity of the allocator. If so, returns `None`.
    - Calculates the number of physical buffers (`count_needed`) required to satisfy the request.
    - Acquires `count_needed` permits from the semaphore. This step is asynchronous and will wait if buffers are currently in use.
    - Pops `count_needed` `UnownedBuffer` instances from the `ArrayQueue`.
    - Constructs a `Slice` from these buffers, passing a clone of the `AllocatorState` (Arc) to the `Slice`.
    - Calls `forget()` on the semaphore permit. This is crucial: the `Slice` takes over the responsibility of returning the permit (via `AllocatorState`) when it is dropped, rather than the permit being released automatically at the end of the function scope.

3.  **Deallocation (`AllocatorState::Drop`)**:
    - When the allocator itself is dropped, it drains any remaining buffers from the pool.
    - Unlocks the memory (`munlock`) if it was locked.
    - Deallocates the original contiguous memory block.

**Edge Cases:**
- **Request Exceeds Capacity**: `allocate` returns `None` immediately without waiting.
- **Pool Exhaustion**: `allocate` awaits the semaphore. If the semaphore is closed (unlikely in this design unless the allocator is broken), it returns `None`.
- **Zero-sized Request**: The `size` parameter is `NonZeroUsize`, so zero-sized requests are prevented at the type level.

**Complexity:**
- **Time**:
  - `new`: O(N) where N is the buffer count (to fill the queue).
  - `allocate`: O(K) where K is the number of buffers needed (to pop from queue). The semaphore acquisition is O(1) amortized.
  - `drop`: O(N) to drain the queue.
- **Space**: O(N * size) for the pre-allocated memory block.

**Determinism:**
- Deterministic. The allocation logic is strictly bounded by the pre-allocated capacity and semaphore state.

---

## 3. Dependency Mechanics

- **`UnownedBuffer` (from `nfs_mamont::allocator::buffer`)**:
  - **Raw Memory Representation**: The allocator relies on `UnownedBuffer` to store pointers to segments of the main memory block. The `Send` and `Sync` traits on `UnownedBuffer` allow these pointers to be safely transferred between threads, which is necessary for the `ArrayQueue` to function in a concurrent environment.
  - **Deref/DerefMut**: While the allocator doesn't directly use these, the returned `Slice` (which wraps `UnownedBuffer`) relies on them to provide data access to the end-user.

- **`Slice` (from `nfs_mamont::allocator::slice`)**:
  - **Lifecycle Management**: The allocator depends on the `Drop` implementation of `Slice`. When the user drops the `Slice`, it automatically pushes the underlying `UnownedBuffer`s back into the `AllocatorState`'s pool and adds permits back to the semaphore. This creates a circular dependency where the allocator produces `Slice`s, and `Slice`s replenish the allocator.
  - **Logical Aggregation**: The allocator uses `Slice` to present a contiguous interface (`Buffer` trait) over potentially multiple disjoint `UnownedBuffer`s, abstracting away the physical fragmentation of the pool.

---

## 4. Data Model

**Entities:**
- **`AllocatorState`**: The shared context containing the memory pool (`ArrayQueue`), the concurrency limiter (`Semaphore`), and metadata about the underlying memory block (`base_ptr`, `layout`).
- **`Impl`**: The public facade of the allocator. It holds an `Arc<AllocatorState>` and configuration parameters (`buffer_size`, `buffer_count`).

**Relations:**
- **`Impl` → `AllocatorState` (1:1)**: `Impl` owns a reference-counted pointer to the state, allowing cheap cloning or sharing if needed (though `Impl` itself is not cloned in the provided code).
- **`AllocatorState` → Memory Block (1:1)**: The state owns a single contiguous allocation of virtual memory.
- **`AllocatorState` → `UnownedBuffer` (1:N)**: The state manages a pool of `UnownedBuffer` instances, each pointing to a sub-region of the main memory block.

**Global Invariants:**
- **Pool Consistency**: The number of items in `pool` plus the number of permits currently held (acquired but not yet released) must equal `buffer_count`.
- **Pointer Validity**: All `UnownedBuffer` instances in the pool must point to valid memory within the range `[base_ptr, base_ptr + layout.size)`.
- **Semaphore Alignment**: The semaphore permit count must accurately reflect the number of buffers available in the pool.

---

## 5. Error Model

**Error Types:**
- None. The module does not define custom error types.

**Error Propagation Strategy:**
- **Panic**: Used for fatal initialization errors (OOM, `mlock` failure).
- **Option**: Used for recoverable runtime conditions (request too large).

**Recoverability:**
- **Initialization**: Not recoverable. Panics on allocation failure or `mlock` failure imply the system cannot start.
- **Allocation**: Recoverable. If `allocate` returns `None`, the caller must handle the lack of memory (e.g., by retrying later or rejecting the operation).

**Panics:**
- **Allowed**: Yes.
- **Conditions**:
  - In `Impl::new`: If the global allocator fails to allocate memory (`alloc::alloc_zeroed` returns null).
  - In `Impl::new`: If `mlock` fails (when the feature is enabled).
  - In `Impl::allocate`: If the semaphore permits acquisition succeeds but the pool is empty (indicates a logic error or race condition in the implementation).

---

## 6. Traits

The module defines and implements the following traits:

- **`Allocator`**: The primary asynchronous interface for acquiring buffers.
  - `allocate(&self, size: NonZeroUsize) -> impl Future<Output = Option<Self::Buffer>> + Send`
- **`Buffer`**: A synchronous interface for accessing the data within an allocated buffer.
  - `chunks(&self)`, `chunks_mut(&mut self)`, `len(&self)`, `is_empty(&self)`, `empty()`
- **`Send`**: Implemented for `AllocatorState` (unsafe), allowing the allocator state to be shared across threads.
- **`Sync`**: Implemented for `AllocatorState` (unsafe), allowing references to the state to be shared across threads.

---

## 7. Overview

This module is used in order to **manage a bounded pool of memory for high-performance, asynchronous I/O operations** within the NFS-Mamont implementation. It solves the problem of unpredictable latency and memory fragmentation associated with dynamic heap allocation in a high-throughput server environment by using a pre-allocated arena.

This system contains a **fixed-capacity, asynchronous memory allocator** that aggregates smaller physical blocks into larger logical buffers. It integrates low-level memory primitives (`UnownedBuffer`), a logical view abstraction (`Slice`), and concurrency control mechanisms (`Semaphore`) to provide a safe and efficient interface for data transmission.

A typical usage scenario of the system involves a network server handling multiple concurrent connections. When the server starts, it initializes the `Impl` allocator, reserving a specific amount of RAM (e.g., 1 GiB) split into fixed chunks (e.g., 64 KiB). When a read or write operation requires a buffer, the server calls `allocate`. If the pool has free capacity, the operation proceeds immediately. If the pool is under heavy load, the semaphore causes the task to wait asynchronously, applying natural backpressure to the network protocol, preventing the server from exhausting system memory.

Inside the system, the following things happen and they use this module:
1.  **Initialization**: The system allocates a large contiguous block of memory. It uses `nfs_mamont::allocator::buffer` to create unsafe, ownership-free handles (`UnownedBuffer`) to sub-regions of this block and stores them in a lock-free queue.
2.  **Allocation**: When a user requests a buffer of arbitrary size, the system calculates how many fixed chunks are needed. It acquires permits from a `tokio::sync::Semaphore` to ensure availability. It then retrieves the chunks from the queue and wraps them in a `Slice` (from `nfs_mamont::allocator::slice`).
3.  **Usage**: The user interacts with the `Slice` via the `Buffer` trait, reading or writing data. The `Slice` abstracts the fact that the data might span multiple non-contiguous physical chunks.
4.  **Reclamation**: Once the I/O operation is complete, the `Slice` is dropped. Its `Drop` implementation automatically returns the physical chunks to the `ArrayQueue` and releases the semaphore permits, making the memory available for future requests.

The critical aspect of this module is the **decoupling of physical memory layout (fixed chunks) from logical usage (variable size slices)**, combined with **asynchronous backpressure** to ensure system stability under load.