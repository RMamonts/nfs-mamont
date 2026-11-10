//! Service-layer implementations of RPC program procedures.
//!
//! This module contains server-side handlers that implement protocol traits
//! declared in higher-level modules (for example, `crate::mount` or `crate::allocator`).

/// Pool-based buffer allocator.
pub mod allocator;

/// MOUNT v3 service implementation.
pub mod mount;

/// NLM v4 service implementation.
pub mod nlm;
