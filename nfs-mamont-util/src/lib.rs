//! Ready-made building blocks for servers built on [`nfs_mamont`].
//!
//! This crate contains server-side implementations of the protocol traits
//! declared in `nfs-mamont` (for example, [`nfs_mamont::mount::Mount`],
//! [`nfs_mamont::nlm::Nlm`] or [`nfs_mamont::Allocator`]). It uses only the public
//! API of `nfs-mamont`, so it also serves as a reference for custom implementations.

// Pool-based buffer allocator.
mod allocator;

// MOUNT v3 service implementation.
mod mount;

// NLM v4 service implementation.
mod nlm;

// Metrics collection for the services.
mod metric;

pub use allocator::{PoolAllocator, Slice};
pub use mount::MountService;
pub use nlm::NlmService;
