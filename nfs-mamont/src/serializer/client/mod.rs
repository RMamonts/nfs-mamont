//! Client-side XDR serializers for NFSv3 and MOUNT request arguments.
//!
//! This module is the counterpart of the server-side serializers (`serializer::server`): it serializes
//! the **arguments** of NFSv3 and MOUNT procedure calls so that a client can build
//! valid requests on the wire. It is gated behind the `client` cargo feature.
//!
//! # Feature
//!
//! Enable with `--features client` in Cargo:
//!
//! ```toml
//! [dependencies.nfs-mamont]
//! features = ["client"]
//! ```
//!
//! With the feature enabled the module is re-exported at the crate root as
//! `nfs_mamont::client`, e.g. `nfs_mamont::client::arguments::nfsv3::access::access_args`.
//! When the feature is disabled this module is not compiled.

pub mod arguments;
