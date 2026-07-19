<!-- SPEC_HASH: 9816971cce9ae6cf15787381e75084f6ea2a3bac3f2be9f4a411b9d13767a2d9 -->
# Module Specification

Module: nfs_mamont::allocator::buffer
Rust File: src/allocator/buffer.rs

---

## 1. Dependencies

From the provided context, there are no external crate dependencies listed. The module relies exclusively on the Rust Standard Library (`std`).

- **`std::ops`**: Used for the `Deref` and `DerefMut` traits. These are implemented to allow the `UnownedBuffer` struct to behave like a standard mutable slice (`&mut [u8]`), enabling ergonomic access to the underlying memory.
- **`std::slice`**: Used within the `Deref` and `DerefMut` implementations to reconstruct slice references (`std::slice::from_raw_parts` and `std::slice::from_raw_parts_mut`) from the raw pointer stored in the struct.

---

## 2. Mechanics

**Intent:**
The module provides a wrapper type `UnownedBuffer` that holds a raw pointer and a length. Its purpose is to represent a mutable view into a memory region without claiming ownership of that memory. This is distinct from standard Rust references (e.g., `&mut [u8]`) because it does not enforce lifetime constraints at the type level, allowing the buffer to be passed around freely or stored in structures where the borrow checker cannot normally verify the validity of the reference. It is designed for low-level memory management scenarios, such as custom allocators or FFI interfaces, where the lifecycle of the memory is managed manually.

**Inputs:**
- A raw pointer (`*mut u8`) pointing to the start of the memory region.
- A length (`usize`) indicating the size of the region in bytes.

**Outputs:**
- A `UnownedBuffer` instance that dereferences to a mutable byte slice (`&mut [u8]`), allowing read and write access to the memory.

**Steps:**
1.  **Construction**: The user calls `UnownedBuffer::from_raw_parts`. This is an `unsafe` operation where the caller guarantees the validity, alignment, and ownership rules of the pointer.
2.  **Storage**: The struct stores the pointer and length.
3.  **Access**: When the buffer is accessed (read or write), the `Deref` or `DerefMut` trait is invoked. This executes `unsafe` code to convert the raw pointer back into a Rust slice reference (`&[u8]` or `&mut [u8]`).
4.  **Destruction**: When the `UnownedBuffer` goes out of scope, it is dropped. Since it does not implement `Drop`, no deallocation occurs. The memory remains allocated, and the responsibility for freeing it stays with the original owner.

**Edge Cases:**
- **Zero-length buffer**: If `len` is 0, the pointer may be dangling or null, but `std::slice::from_raw_parts` generally allows this for zero-sized slices.
- **Aliasing**: Because `DerefMut` is implemented, multiple `UnownedBuffer` instances could theoretically point to the same memory region. Rust's safety rules regarding mutable aliasing are bypassed here; creating two mutable references to the same memory via two `UnownedBuffer` instances constitutes Undefined Behavior unless synchronized externally.

**Complexity:**
- **Time**: O(1) for creation and access (dereferencing is a direct pointer cast).
- **Space**: O(1) stack space for the struct (size of a pointer and a `usize`).

**Determinism:**
- Deterministic. The behavior is entirely defined by the pointer value and length provided at construction.

---

## 3. Dependency Mechanics

There are no external module dependencies provided in the context. The module relies on standard Rust mechanisms:
- **Trait Implementation (`Deref`/`DerefMut`)**: The module leverages Rust's dereferencing operators to transparently convert raw memory representations into safe slice interfaces at the point of use.

---

## 4. Data Model

**Entities:**
- **`UnownedBuffer`**: A container holding a raw pointer to a byte array (`*mut u8`) and its length (`usize`).

**Relations:**
- **UnownedBuffer → Memory Region (1:1)**: The struct refers to a specific contiguous block of memory. This relationship is not tracked by the Rust compiler (no lifetime parameter).

**Global Invariants:**
- **Pointer Validity**: The `ptr` must point to a valid memory allocation of at least `len` bytes for the entire duration the `UnownedBuffer` exists.
- **Alignment**: The `ptr` must be aligned for `u8` (which is always true for any allocation).
- **Ownership**: The `UnownedBuffer` must not be used to deallocate the memory. The deallocation must happen exactly once via the original allocator/owner.
- **Exclusivity**: If a mutable reference (`&mut [u8]`) is obtained via `DerefMut`, no other active references (mutable or shared) to the same memory range should exist simultaneously to avoid data races.

---

## 5. Error Model

**Error Types:**
- None. The module does not define or return error types (e.g., `Result`).

**Error Propagation Strategy:**
- N/A.

**Recoverability:**
- N/A.

**Panics:**
- **Allowed**: No explicit panics are triggered by the code in this module.
- **Conditions**: While the code itself does not panic, invalid usage (e.g., passing a null pointer for a non-zero length) leads to Undefined Behavior rather than a guaranteed panic. `std::slice::from_raw_parts` may panic if the memory is not aligned or if the pointer is invalid in specific debug configurations, but in release, it typically results in segmentation faults or silent corruption.

---

## 6. Traits

The module implements the following external traits for `UnownedBuffer`:
- **`std::fmt::Debug`**: Automatically derived, allowing the struct to be formatted for debugging output.
- **`std::ops::Deref`**: Targets `&[u8]`, allowing read-only access to the buffer as a slice.
- **`std::ops::DerefMut`**: Targets `&mut [u8]`, allowing read-write access to the buffer as a mutable slice.
- **`Send`**: Implemented `unsafe`ly. Indicates the struct can be transferred between threads. This is safe because the struct is essentially a `usize` and a pointer; thread safety depends on the underlying memory, not the struct itself.
- **`Sync`**: Implemented `unsafe`ly. Indicates a reference to the struct can be shared between threads. Similar to `Send`, this delegates safety to the underlying memory management.

---

## 7. Overview

This module is used in order to decouple the *access* to a memory buffer from the *ownership* and *lifetime* of that memory. In the context of the `nfs_mamont::allocator` system, this module provides a fundamental primitive for handling raw memory blocks returned by a custom allocator.

This system contains low-level memory management utilities. A typical usage scenario involves a custom allocator that hands out a region of memory to a caller. Instead of using standard Rust references—which would tie the allocator's internal state to complex lifetimes or prevent the allocator from mutating its own metadata while the buffer is in use—the allocator returns an `UnownedBuffer`. This struct wraps the raw pointer. The caller can then read and write to this buffer as if it were a normal byte slice (`&mut [u8]`), thanks to the `Deref` and `DerefMut` implementations.

Inside the system, the following things happen and they use this module:
1.  **Allocation**: An allocator reserves a block of memory.
2.  **Handle Creation**: The allocator creates an `UnownedBuffer` pointing to this block.
3.  **Usage**: The user of the buffer modifies the data via the buffer interface.
4.  **Deallocation**: The user eventually returns the buffer (or just drops it), and the allocator reclaims the memory using its own logic, independent of the `UnownedBuffer`'s destructor.

The critical aspect of `UnownedBuffer` is that it shifts the burden of safety to the caller. The system relies on this to achieve performance or flexibility (e.g., zero-copy networking, inter-process shared memory) that safe Rust abstractions might restrict. The `Send` and `Sync` implementations allow these buffers to be passed between threads, assuming the underlying memory region supports such access.