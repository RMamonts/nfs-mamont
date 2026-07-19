<!-- SPEC_HASH: 9adb9413e988edad697cf97e0aad732503b34a29fc4f0a0d30f3e382ffd594bf -->
# Module Specification

Module: mirrorfs::args 
Rust File: src/args.rs

---

## 1. Dependencies

From the code analysis, the following external crates and modules are used:

- **`std::net::SocketAddr`**: Used to store and validate the IP address and port number provided by the user. It ensures that the `addr` field holds a valid network endpoint.
- **`std::path::PathBuf`**: Used to store the file system path to the configuration file. It provides an owned, mutable path handle.
- **`clap::Parser`**: A third-party crate (derive macro) used to generate command-line argument parsing logic from the `Args` struct definition. It automates the mapping of string arguments from `std::env::args` to the typed fields of the struct.

*Assumption:* Since specifications for `std` and `clap` were not provided in the context, standard behavior for `clap` v3/v4 is assumed (parsing `argv`, exiting on error, handling help flags).

---

## 2. Mechanics

**Intent:**
To define a schema for command-line arguments that initializes the `mirrorfs` server. This module bridges the gap between the user's shell invocation and the application's internal configuration requirements by enforcing types, providing defaults, and generating help documentation.

**Inputs:**
- Command-line arguments passed to the executable (typically `std::env::args`).
- Specifically:
  - A flag `-c` or `--config` followed by a file path.
  - A flag `-a` or `--addr` (inferred from field name `addr` due to `#[arg(short, long)]`) followed by a socket address.

**Outputs:**
- An instantiated `Args` struct containing:
  - `config_path`: A `PathBuf` pointing to the TOML configuration file.
  - `addr`: A `SocketAddr` representing the listening interface and port.

**Steps:**
1. The `clap::Parser` derive macro processes the `Args` struct definition at compile time to generate a parser.
2. At runtime, the parser inspects the command-line arguments provided to the process.
3. It attempts to match the `-c` or `--config` argument to the `config_path` field. If present, the value is parsed into a `PathBuf`. If missing, the parser reports an error (required field).
4. It attempts to match the `-a` or `--addr` argument to the `addr` field. If present, the value is parsed into a `SocketAddr`. If missing, the default value `"0.0.0.0:2049"` is used.
5. If parsing fails (e.g., invalid IP format), the parser prints an error message and terminates the process.
6. If parsing succeeds, the `Args` struct is returned to the caller.

**Edge Cases:**
- **Invalid Socket Address:** If the user provides a string that cannot be parsed as a `SocketAddr` (e.g., "localhost:abc"), `clap` will emit a parsing error and exit.
- **Missing Config:** If the `-c` argument is omitted, `clap` will report a required argument error and exit.
- **Path Handling:** The `value_hint = clap::ValueHint::AnyPath` suggests the shell may offer path completion, but the module itself does not check for file existence; it only accepts the string as a path.

**Complexity:**
- **Time:** O(N), where N is the number of command-line arguments. This is dominated by the `clap` library's parsing logic.
- **Space:** O(1) regarding the input size (allocating only the `PathBuf` and `SocketAddr`).

**Determinism:**
- **Deterministic.** Given the same set of input command-line arguments, the resulting `Args` struct will always be identical.

---

## 3. Dependency Mechanics

*Note: No dependency specifications were provided in the context. The following are assumptions based on general knowledge of the dependencies used.*

- **`clap::Parser`**:
  - **Mechanism**: Derive macro expansion.
  - **Importance**: It is the core mechanism that transforms the struct definition into a functional parser. It handles the mapping of short/long flags to struct fields, type validation (parsing strings to `SocketAddr`), and default value injection.

---

## 4. Data Model

**Entities:**
- **`Args`**: The configuration container.
  - `config_path`: `PathBuf` — Represents the location of the TOML configuration file.
  - `addr`: `SocketAddr` — Represents the network endpoint (IP and Port) for the NFS server.

**Relations:**
- None. The `Args` struct is a flat data structure.

**Global Invariants:**
- `config_path` must be non-empty (enforced by `clap` as a required argument).
- `addr` must be a valid IPv4 or IPv6 address with a port. If not provided by the user, it defaults to `0.0.0.0:2049`.

---

## 5. Error Model

**Error Types:**
- **Parse Errors**: Occur when the provided string for `addr` does not conform to a valid `SocketAddr` format.
- **Missing Argument Errors**: Occur when the required `config_path` argument is not provided.

**Error Propagation Strategy:**
- **`clap` default behavior**: The `clap` library, when used via the `Parser` trait, typically handles errors by printing a user-friendly message to `stderr` and terminating the process with a non-zero exit code. It does not return a `Result<T, E>` to the caller in the standard flow.

**Recoverability:**
- **Non-recoverable**. If arguments are invalid or missing, the program cannot proceed to start the server and must exit.

**Panics:**
- **Allowed**: No.
- **Conditions**: The code in this module contains no explicit panic points. Panics would only occur if the underlying `clap` library or `std` parsing logic encounters an unexpected state (a bug in the dependencies, not this module).

---

## 6. Traits

List which external traits this module implements:
- **`clap::Parser`**: Implemented via `#[derive(Parser)]`. This trait allows the struct to be parsed from command-line arguments.

---

## 7. Overview

This module is used in order to **encapsulate and validate the entry-point configuration** for the `mirrorfs` NFS server. It serves as the single source of truth for the interface between the system administrator (or the calling environment) and the application's runtime logic.

This system contains **a command-line interface definition** that abstracts away the raw string processing of `std::env::args`. By using `clap`, it ensures that type safety is enforced at the boundary of the application (e.g., ensuring the address is a valid `SocketAddr` before the rest of the application logic runs).

A typical usage scenario of the system involves a user invoking the binary, for example: `./mirrorfs -c /etc/mirrorfs.toml --addr 192.168.1.10:2050`. The `Args` struct captures these inputs.

Inside the system the following things happen and they use **the `Args` struct to bootstrap the application**:
1. The `main` function (or entry point) calls `Args::parse()`.
2. The `config_path` is passed to a configuration loader (likely reading the TOML file mentioned in the docstring).
3. The `addr` is passed to a TCP listener or NFS service initializer to bind to the specific network interface.

Without this module, the application would lack a standardized way to accept these critical parameters, leading to potential runtime errors if invalid types were passed deep into the network or file system logic.