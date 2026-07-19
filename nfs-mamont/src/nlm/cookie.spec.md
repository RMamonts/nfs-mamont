<!-- SPEC_HASH: caaa1302b6a589c83addfe7e0576a2ee4c0385c8833bfab364b4e42671b903e7 -->
# Module Specification

Module: nfs_mamont::nlm::cookie
Rust File: src/nlm/cookie.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

*   **Standard Library (`core` / `std`)**:
    *   `u64`: Used as the underlying storage type for the directory position marker.
    *   Derive macros (`Clone`, `Copy`, `Debug`, `PartialEq`, `Eq`): Used to implement common traits for the `Cookie` struct, allowing it to be easily copied, compared, and debugged.
*   **Assumption**: No external crate dependencies or internal module dependencies are explicitly imported in the provided source code. The module relies solely on Rust built-in types.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module.
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To provide a type-safe wrapper around a raw `u64` value that represents a cursor or bookmark within a directory listing (specifically for the NFS `READDIR` protocol). This prevents confusion with other `u64` values in the system (e.g., file sizes, offsets) and encapsulates the logic for checking the start of a directory.

Inputs:
- `val: u64`: A raw unsigned 64-bit integer representing a specific point in a directory, typically obtained from a server response.
- `self`: An instance of `Cookie` for inspection.

Outputs:
- `Cookie`: A new instance wrapping the provided value.
- `u64`: The raw internal value of the cookie.
- `bool`: A flag indicating if the cookie represents the initial position (zero).

Steps:
1.  **Construction**: The `new` function takes a `u64` and stores it in the `Cookie` struct's tuple field.
2.  **Extraction**: The `raw` function returns the internal `u64` value, likely required when constructing network packets or protocol messages.
3.  **Validation**: The `is_zero` function compares the internal value to `0`. In the context of NFS `READDIR`, a cookie of `0` signifies the start of the directory.

Edge Cases:
- **Zero Cookie**: Represents the beginning of a directory stream. The `is_zero` method specifically handles this semantic check.
- **Maximum Value**: The struct accepts any `u64`, including the maximum value (`u64::MAX`), which is valid as a protocol value.

Complexity:
- Time: O(1) for all operations (simple field access or comparison).
- Space: O(1) (size of `u64`, 8 bytes).

Determinism:
- Deterministic

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- N/A (No external dependencies with specifications provided).

---

## 4. Data Model

Entities:
- `Cookie`: A newtype wrapper around a `u64` representing a directory entry marker.

Relations:
- None (Single entity module).

Global Invariants:
- The internal value `0` is semantically reserved to indicate the start of a directory listing.
- The struct is `Copy` and `Clone`, implying it should be treated as a simple, pass-by-value value type rather than a managed resource.

---

## 5. Error Model

Error Types:
- None. The module does not define any error types, and the methods do not return `Result` types.

Error Propagation Strategy:
- N/A (No error propagation).

Recoverability:
- N/A.

Panics:
- Allowed: No.
- Conditions: None. The code consists of simple struct field access and integer comparison, which cannot panic.

---

## 6. Traits

List which external traits this module implements:
- `Clone`: Allows creating duplicates of the `Cookie` instance.
- `Copy`: Allows implicit bitwise copying during assignment.
- `Debug`: Enables formatting the structure using `{:?}`.
- `PartialEq`: Enables equality comparison (`==`) between `Cookie` instances.
- `Eq`: Marks the type as an equivalence relation (total equality).

---

## 7. Overview

This section is needed for the evolution of project understanding.
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module.
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed.
Here you MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module.
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level.
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to enforce type safety and semantic clarity when handling directory stream cursors in a Network File System (NFS) implementation. In the NFS protocol, directory listings (`READDIR`) are stateful; the server returns a list of entries along with a "cookie" (a verifier) for each entry. The client must return this specific cookie to the server to resume reading from that exact point.

This system contains a primitive abstraction layer that isolates the raw numeric values used in network protocols from the application logic. By wrapping the `u64` in a `Cookie` struct, the system prevents "primitive obsession," ensuring that a directory position marker cannot be accidentally confused with other unsigned integers, such as file sizes, inode numbers, or timestamps.

A typical usage scenario of the system involves a client requesting a directory listing. The client initializes the request with `Cookie::new(0)` (checked via `is_zero`). Upon receiving a response, the client extracts the cookie associated with the last entry in the batch using `Cookie::new(val_from_server)`. This `Cookie` is then stored and used in the subsequent request to fetch the next batch of entries.

Inside the system the following things happen and they use the `Cookie` type to validate that the cursor passed to the `READDIR` handler is indeed a directory position and not an arbitrary integer, thus reducing the surface area for API misuse.