//! RPC (ONC RPC) XDR serializers.
//!
//! Contains helpers for encoding RPC-level structures (for example, opaque
//! authentication credentials/verifiers).

use std::io;
use std::io::Write;

use crate::rpc::{OpaqueAuth, MAX_AUTH_SIZE};
use crate::serializer::{variant, vec_max_size};

/// Serializes [`OpaqueAuth`] (flavor + body) into XDR.
pub fn auth_opaque(dest: &mut impl Write, data: OpaqueAuth) -> io::Result<()> {
    variant(dest, data.flavor)?;
    vec_max_size(dest, data.body, MAX_AUTH_SIZE)
}
