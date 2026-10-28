//! Service-layer implementations of RPC program procedures.
//!
//! This module contains server-side handlers that implement protocol traits
//! declared in higher-level modules (for example, `crate::mount`).

/// MOUNT v3 service implementation.
#[allow(unused)]
pub mod mount;

/// NLM v4 service implementation.
#[allow(unused)]
pub mod nlm;
