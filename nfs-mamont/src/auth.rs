//! High-level types representing RPC authentication and authorization.
//!
//! This module exposes the parsed, authenticated caller identity that the
//! server works with once a request's RPC header has been validated. It is the
//! *high-level* counterpart to the low-level decoding performed by the RPC
//! parser, and is what gets handed to the NFS/MOUNT services via [`Credential`]
//! so they can enforce per-caller permissions.

use num_derive::{FromPrimitive, ToPrimitive};

/// Authentication status codes (`auth_stat`) from RFC 5531, section 5.3.3.
///
/// See RFC 5531, for the canonical descriptions.
#[derive(Debug, PartialEq, PartialOrd, ToPrimitive, FromPrimitive)]
pub enum AuthStat {
    /// The call succeeded and the credentials were accepted. `AUTH_OK`.
    Ok = 0,
    /// The credential was malformed or its seal was broken. `AUTH_BADCRED`.
    BadCred = 1,
    /// The credential expired or is otherwise no longer valid. `AUTH_REJECTEDCRED`.
    RejectedCred = 2,
    /// The verifier sent, e.g. the time stamp it is sealed with, is invalid.
    /// `AUTH_BADVERF`.
    BadVerf = 3,
    /// The verifier has expired. `AUTH_REJECTEDVERF`.
    RejectedVerf = 4,
    /// The caller could not obtain the server's credentials, so the encryption
    /// level it used is not strong enough. `AUTH_TOOWEAK`.
    TooWeak = 5,
    /// The verifier did not match the response it was accompanying.
    /// `AUTH_INVALIDRESP`.
    InvalidResp = 6,
    /// Some failure that does not fit any of the other codes, usually a
    /// protocol error on the server side. `AUTH_FAILED`.
    Failed = 7,
    /// A generic Kerberos failure occurred while processing the credential.
    /// `AUTH_KERB_GENERIC`.
    KerbGeneric = 8,
    /// The Kerberos ticket used in the credential has expired.
    /// `AUTH_TIME_EXPIRE`.
    TimeExpire = 9,
    /// There was a problem opening the user's Kerberos ticket file.
    /// `AUTH_TKT_FILE`.
    TktFile = 10,
    /// The credential could not be decoded. `AUTH_DECODE`.
    Decode = 11,
    /// The caller's network address is unknown. `AUTH_NET_ADDR`.
    NetAddr = 12,
    /// RPCSEC_GSS-specific: a problem with the GSS-API credential.
    /// `AUTH_RPCSEC_GSS_CREDPROBLEM`.
    RpcSecGssCredProblem = 13,
    /// RPCSEC_GSS-specific: a problem with the GSS-API context.
    /// `AUTH_RPCSEC_GSS_CTXPROBLEM`.
    RpcSecGssCtxProblem = 14,
}

/// Parsed `AUTH_SYS` credential body (`authsys_parms`, RFC 5531 appendix A).
///
/// This is the decoded form of the `opaque_auth` body that accompanies an
/// `AUTH_SYS` flavor. It identifies the caller the same way a UNIX process does:
/// by machine name and a caller-supplied uid/gid pair plus a list of auxiliary
/// groups.
#[derive(Debug, Clone)]
#[cfg_attr(test, derive(PartialEq))]
pub struct AuthSysParams {
    /// Arbitrary ID the caller stamps on the credential; meaningful only to the caller.
    pub stamp: u32,
    /// Name of the caller's machine, at most 255 bytes.
    pub machine_name: String,
    /// Effective user ID of the caller.
    pub uid: u32,
    /// Effective group ID of the caller.
    pub gid: u32,
    /// Auxiliary group IDs of the caller — at most 16 entries.
    pub gids: Vec<u32>,
}

/// Authenticated caller identity extracted from an RPC credential.
///
/// Only the flavors the server accepts are represented: `AUTH_NONE` (anonymous)
/// and `AUTH_SYS` (UNIX-style uid/gid). Any other flavor is rejected during
/// parsing with [`AuthStat::BadCred`].
///
/// This value is passed down to the service layer (e.g. every
/// [`crate::vfs::Vfs`] method and the MOUNT `mnt` procedure) so an
/// implementation can make authorization decisions based on the caller.
#[derive(Debug, Clone)]
#[cfg_attr(test, derive(PartialEq))]
pub enum Credential {
    /// `AUTH_NONE`: anonymous caller, no identity provided.
    None,
    /// `AUTH_SYS`: UNIX-style caller identity.
    Sys(AuthSysParams),
}
