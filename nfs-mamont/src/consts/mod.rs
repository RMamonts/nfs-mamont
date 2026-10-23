//! Protocol constants used by the server.
//!
//! Each submodule groups the constants of one protocol: program and version
//! numbers, procedure numbers and size limits taken from the corresponding
//! specification.

pub mod mount;
pub mod nfsv3;
pub mod nlm;
pub mod rpc;
pub mod socket;
pub mod xdr;
