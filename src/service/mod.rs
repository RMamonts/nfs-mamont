//! Service-layer implementations of RPC program procedures.
//!
//! This module contains server-side handlers that implement protocol traits
//! declared in higher-level modules (for example, `crate::interface::mount`).

/// MOUNT v3 service implementation.
mod mount;
