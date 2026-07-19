<!-- SPEC_HASH: 4590b0f9584d14ca1b6ccfabef16bcad0480b0beb038fccb7219e7512902130a -->
# Module Specification

Module: nfs_mamont::allocator
Rust File: src/allocator/mod.rs

---

## 1. Dependencies

From the provided context and dependency facts:

- **`std::alloc`**: Used for low-level memory management. Specifically, `alloc::alloc_zeroed` is used to reserve a contiguous block of memory for the entire pool at initialization, and `alloc::dealloc` is used to free this block when the allocator is dropped.
- **`crossbeam_queue::ArrayQueue`**: A lock-free concurrent queue used to implement the buffer pool (`AllocatorState.pool`). It stores available `UnownedBuffer` instances, allowing multiple threads to acquire and return buffers without heavy locking overhead.
- **`tokio::sync::Semaphore`**: An asynchronous semaphore used to limit the number of buffers currently in use. It provides backpressure by making `allocate` wait until a buffer is available if the pool is exhausted.
- **`nfs_mamont::allocator::buffer`**: Provides the `UnownedBuffer` type, which serves as the physical unit of memory in the pool. The allocator splits its large contiguous memory block into multiple `UnownedBuffer` instances.
- **`nfs_mamont::allocator::slice`**: Provides the `Slice` type, which implements the `Buffer` trait. The allocator returns `Slice` instances to users, aggregating multiple `UnownedBuffer`s into a single logical view.
- **`libc`** (feature `mlock`): Used for `mlock` and `munlock` system calls to prevent the allocated memory from being swapped to disk, ensuring predictable performance.

---

## 2. Mechanics

**Intent:**
The module implements a bounded, asynchronous memory pool allocator. Its primary goal is to pre-allocate a fixed amount of memory at startup and distribute it in fixed-size chunks to satisfy variable-sized allocation requests. This design eliminates runtime allocation latency, prevents memory fragmentation, and enforces strict memory usage limits via a semaphore.

**Inputs:**
- **Configuration (`Impl::new`)**:
 - `size`: Size of a single physical buffer chunk (`NonZeroUsize`).
 - `count`: Total number of chunks to pre-allocate (`NonZeroUsize`).
- **Allocation Request (`Impl::allocate`)**:
 - `size`: The minimum number of bytes required for the allocation (`NonZeroUsize`).

**Outputs:**
- **Allocator Instance**: A configured `Impl` ready to serve requests.
- **Buffer Handle**: An asynchronous `Future` resolving to `Option<Slice>`. `Some(Slice)` contains the allocated memory, `None` indicates the request exceeds total capacity.

**Steps:**

1. **Initialization (`Impl::new`)**:
 - Calculates the total memory required (`size * count`).
 - Creates a memory `Layout` for the total block.
 - Allocates a single contiguous block of zeroed memory using `std::alloc::alloc_zeroed`.
 - Optionally calls `mlock` on the block to prevent swapping.
 - Iterates over the memory block, creating `count` `UnownedBuffer` instances, each pointing to a sub-region of the block.
 - Pushes all `UnownedBuffer` instances into an `ArrayQueue` (the pool).
 - Initializes a `Semaphore` with `count` permits.
 - Wraps the pool, semaphore, and memory metadata in `Arc<AllocatorState>`.

2. **Allocation (`Impl::allocate`)**:
 - Checks if the requested `size` exceeds the total capacity of the allocator. If so, returns `None`.
 - Calculates the number of physical chunks (`count_needed`) required to satisfy the request (ceiling division).
 - Acquires `count_needed` permits from the semaphore. This operation is asynchronous and will wait if insufficient permits are available.
 - Pops `count_needed` `UnownedBuffer` instances from the `ArrayQueue`.
 - Constructs a `Slice` from the collected buffers, defining a range covering the requested `size`.
 - Passes a clone of the `Arc<AllocatorState>` to the `Slice`.
 - Calls `forget()` on the semaphore permits. This transfers the responsibility of releasing the permits to the `Slice` (via its `Drop` implementation), preventing the semaphore from releasing them prematurely when the `allocate` future completes.
 - Returns `Some(Slice)`.

3. **Deallocation (Implicit via `Slice` Drop)**:
 - When the user drops the `Slice`, the `slice` module's `Drop` implementation triggers.
 - It pushes the underlying `UnownedBuffer`s back into the `AllocatorState.pool`.
 - It adds the corresponding number of permits back to the `AllocatorState.semaphore`.

4. **Destruction (`AllocatorState::drop`)**:
 - Drains the `ArrayQueue` (dropping all `UnownedBuffer` handles).
 - Optionally calls `munlock`.
 - Deallocates the original contiguous memory block using `std::alloc::dealloc`.

**Edge Cases:**
- **Oversized Request**: If `size` > total capacity, `allocate` returns `None` immediately without waiting.
- **Allocation Failure**: If `alloc_zeroed` returns a null pointer during initialization, the process aborts via `handle_alloc_error`.
- **mlock Failure**: If `mlock` fails (e.g., insufficient privileges), the constructor panics.

**Complexity:**
- **Time**:
 - `new`: O(N) where N is the buffer count (to populate the queue).
 - `allocate`: O(K) where K is the number of chunks needed (to pop from queue). The semaphore acquisition is O(1) amortized.
 - `drop`: O(N) to drain the queue.
- **Space**: O(Total Memory) for the contiguous block + O(N) for the queue metadata.

**Determinism:**
- Deterministic. The allocation logic is strictly bounded by the pre-allocated capacity and semaphore state.

---

## 3. Dependency Mechanics

- **`UnownedBuffer` (from `nfs_mamont::allocator::buffer`)**:
 - **Physical Storage**: The allocator relies on `UnownedBuffer` to represent the fixed-size chunks of the pre-allocated memory block. The `ArrayQueue` stores these buffers, and the `Slice` consumes them. The `Send` and `Sync` traits on `UnownedBuffer` are crucial for allowing the allocator to be shared across threads safely.

- **`Slice` (from `nfs_mamont::allocator::slice`)**:
 - **Logical Aggregation & RAII**: The allocator uses `Slice` to present a contiguous interface over potentially disjoint physical chunks. Critically, the allocator depends on `Slice`'s `Drop` implementation to manage the lifecycle of the semaphore permits. By passing `Arc<AllocatorState>` to the `Slice`, the allocator delegates the task of returning buffers to the pool and releasing semaphore permits to the `Slice` itself, ensuring resources are reclaimed exactly once when the user is done with the data.

---

## 4. Data Model

**Entities:**
- **`AllocatorState`**: The shared context containing the memory pool (`ArrayQueue`), concurrency limiter (`Semaphore`), and the pointer/layout of the backing memory.
- **`Impl`**: The public facade of the allocator. Holds configuration (buffer size, count) and a reference-counted pointer to the `AllocatorState`.
- **`Buffer` (Trait)**: The interface defining how users interact with allocated memory (read/write chunks).
- **`Allocator` (Trait)**: The interface defining the asynchronous allocation contract.

**Relations:**
- **`Impl` → `AllocatorState` (1:1)**: `Impl` owns an `Arc<AllocatorState>`.
- **`AllocatorState` → Memory Block (1:1)**: The state owns the raw pointer to the single contiguous allocation.
- **`AllocatorState` → `UnownedBuffer` (1:N)**: The pool manages N `UnownedBuffer` instances that slice into the memory block.
- **`Slice` → `AllocatorState` (N:1)**: Multiple active `Slice` instances may hold a reference to the same `AllocatorState` to return resources upon drop.

**Global Invariants:**
- **Capacity Invariant**: The total number of bytes available in the pool is `buffer_size * buffer_count`.
- **Semaphore-Pool Consistency**: The number of permits available in the semaphore plus the number of buffers currently held in active `Slice`s must equal the total buffer count. (i.e., `semaphore.available_permits() + active_buffers == total_count`).
- **Pointer Validity**: `base_ptr` in `AllocatorState` must remain valid and aligned for the lifetime of the `AllocatorState`.

---

## 5. Error Model

**Error Types:**
- None defined explicitly in the module. Errors are handled via panics or `Option` returns.

**Error Propagation Strategy:**
- **`Option`**: Used in `allocate` to indicate that a request is too large for the allocator's capacity.
- **Panic**: Used for unrecoverable initialization errors (OOM, `mlock` failure).

**Recoverability:**
- **Initialization**: Not recoverable. Panics in `new` (e.g., `mlock` failure) terminate the task or thread.
- **Allocation**: Recoverable. If `allocate` returns `None`, the caller must handle the failure (e.g., reject the request).

**Panics:**
- **Allowed**: Yes.
- **Conditions**:
 - In `Impl::new`: If the system allocator fails to provide memory (`alloc::handle_alloc_error`).
 - In `Impl::new`: If `mlock` fails (when feature is enabled).
 - In `Impl::allocate`: If the semaphore is closed (unlikely in this design unless the `AllocatorState` is dropped while futures are pending, which would imply a logic error in the lifecycle management).

---

## 6. Traits

The module defines the following traits:
- **`Buffer`**: Defines the interface for accessing allocated memory. Requires `Send` and `Sync`. Methods: `chunks`, `chunks_mut`, `len`, `is_empty`, `empty`.
- **`Allocator`**: Defines the interface for asynchronous memory allocation. Associated type `Buffer`. Method: `allocate`.

The module implements the following traits:
- **`Allocator`** for `Impl`.
- **`Drop`** for `AllocatorState`.
- **`Send`** and **`Sync`** for `AllocatorState` (unsafe impl).

---

## 7. Overview

This module is used in order to **provide a bounded, zero-allocation-overhead memory source for high-performance network I/O** within the NFS-Mamont implementation. It solves the problem of unpredictable latency and memory fragmentation associated with dynamic heap allocation by pre-allocating a fixed pool of memory at startup.

This system contains a **fixed-capacity memory pool** that manages a contiguous block of RAM. It splits this block into fixed-size "pages" (`UnownedBuffer`) and uses a semaphore to enforce strict limits on concurrent usage. The system allows users to request buffers of arbitrary sizes; if a request is larger than a single page, the allocator aggregates multiple pages into a single logical view (`Slice`).

A typical usage scenario of the system involves a network server handling incoming data. The server creates an `Impl` allocator with, for example, 1000 buffers of 4KB each. When a client sends a 10KB message, the server calls `allocate(10KB)`. The system calculates that 3 pages are needed, acquires 3 semaphore permits, pops 3 `UnownedBuffer`s from the pool, and wraps them in a `Slice`. The server reads the network data directly into the `Slice`. Once the message is processed, the `Slice` is dropped, automatically returning the 3 pages to the pool and releasing the semaphore permits, making them available for the next connection.

Inside the system, the following things happen and they use this module:
1. **Initialization**: The application bootstraps the allocator. `Impl::new` reserves a large chunk of virtual memory (and optionally locks it via `mlock`), ensuring that the memory footprint is known and constant.
2. **Backpressure Handling**: During high load, if all buffers are in use, the `Semaphore` in `AllocatorState` causes the `allocate` future to yield. This naturally applies backpressure to the network layer, preventing the server from accepting more work than it can handle in memory.
3. **Resource Lifecycle Management**: The module relies on the `Slice` dependency to handle the complex logic of returning memory. By "forgetting" the semaphore permits in `allocate` and relying on `Slice` to re-add them in its `Drop` implementation, the system ensures that permits accurately reflect the number of buffers currently allocated to user code, even across asynchronous task boundaries.

The critical aspect of this module is the **decoupling of physical memory layout (contiguous block, fixed chunks) from the logical memory interface (variable-sized `Slice`)**, facilitated by a lock-free pool and an asynchronous semaphore for concurrency control.