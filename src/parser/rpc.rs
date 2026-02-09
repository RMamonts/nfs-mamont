//! Implements parsing for RPC (Remote Procedure Call) related structures.

use std::io::Read;

use num_derive::FromPrimitive;

use crate::parser::primitive::{variant, vec_max_size};
use crate::parser::Result;

const MAX_AUTH_SIZE: usize = 400;

/// Represents a parsed RPC message with program, procedure, and version.
#[derive(Debug)]
pub(super) struct RpcMessage {
    pub(super) program: u32,
    pub(super) procedure: u32,
    pub(super) version: u32,
}

/// Enumerates possible authentication status values.
#[derive(Debug, PartialOrd, PartialEq)]
pub enum AuthStat {
    /// Authentication was successful.
    AuthOk = 0,
    /// Bad credential (syntax error for example).
    AuthBadCred = 1,
    /// Client credential too weak or not accepted by server.
    AuthRejectedCred = 2,
    /// Bogus verifier (bad opaque field).
    AuthBaDVerf = 3,
    /// Verifier expired or not accepted.
    AuthRejectedVerf = 4,
    /// Client not eligible for secure authentication.
    AuthTooWeak = 5,
    /// Bogus response verifier.
    AuthInvalidResp = 6,
    /// Some unknown authentication error occurred.
    AuthFailed = 7,
    /// Kerberos specific error.
    AuthKerbGeneric = 8,
    /// Kerberos time expired.
    AuthTimeExpire = 9,
    /// Kerberos ticket file error.
    AuthTktFile = 10,
    /// Kerberos decode error.
    AuthDecode = 11,
    /// Kerberos net address error.
    AuthNetAddr = 12,
    /// RPCSEC_GSS credential problem.
    RpcSecGssCredProblem = 13,
    /// RPCSEC_GSS context problem.
    RpcSecGssCtxProblem = 14,
}

/// Enumerates possible authentication flavors.
#[derive(FromPrimitive)]
pub enum AuthFlavor {
    /// No authentication.
    AuthNone = 0,
    /// Standard UNIX authentication.
    AuthSys = 1,
    /// Des authentication.
    AuthShort = 2,
    /// Diffie-Hellman authentication.
    AuthDh = 3,
    /// RPCSEC_GSS authentication.
    RpcSecGss = 6,
}

/// Represents opaque authentication data.
#[allow(dead_code)]
pub struct OpaqueAuth {
    pub flavor: AuthFlavor,
    pub opaque: Vec<u8>,
}

/// Parses an [`OpaqueAuth`] structure from the provided `Read` source.
pub fn auth(src: &mut impl Read) -> Result<OpaqueAuth> {
    Ok(OpaqueAuth {
        flavor: variant::<AuthFlavor>(src)?,
        opaque: vec_max_size(src, MAX_AUTH_SIZE)?,
    })
}
