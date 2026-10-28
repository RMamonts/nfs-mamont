//! Client-side serializers for the arguments of individual procedure calls.
//!
//! Mirroring the structure of [`crate::serializer::server`], each submodule here
//! serializes the request arguments of one NFSv3 or MOUNT procedure:
//!
//! - [`mount`] — the MOUNT protocol (`MOUNT`, `UNMOUNT`).
//! - [`nfsv3`] — the NFSv3 protocol procedures.

pub mod mount;
pub mod nfsv3;
