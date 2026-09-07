use std::io;
use std::string::FromUtf8Error;

use num_derive::{FromPrimitive, ToPrimitive};

pub const RPC_VERSION: u32 = 2;

pub const MAX_AUTH_SIZE: usize = 400;

/// Maximum length of the `machinename` field in `AUTH_SYS` credentials (RFC 5531, appendix A).
pub const AUTH_SYS_MAX_MACHINE_NAME: usize = 255;

/// Maximum number of auxiliary GIDs in `AUTH_SYS` credentials (RFC 5531, appendix A).
pub const AUTH_SYS_MAX_GIDS: usize = 16;

#[derive(ToPrimitive, FromPrimitive)]
pub enum AcceptStat {
    Success = 0,
    ProgUnavail = 1,
    ProgMismatch = 2,
    ProcUnavail = 3,
    GarbageArgs = 4,
    SystemErr = 5,
}

#[derive(Debug, PartialEq, PartialOrd, ToPrimitive, FromPrimitive)]
pub enum AuthStat {
    Ok = 0,
    BadCred = 1,
    RejectedCred = 2,
    BadVerf = 3,
    RejectedVerf = 4,
    TooWeak = 5,
    InvalidResp = 6,
    Failed = 7,
    KerbGeneric = 8,
    TimeExpire = 9,
    TktFile = 10,
    Decode = 11,
    NetAddr = 12,
    RpcSecGssCredProblem = 13,
    RpcSecGssCtxProblem = 14,
}

#[derive(ToPrimitive, FromPrimitive)]
pub enum RpcBody {
    Call = 0,
    Reply = 1,
}

pub enum ReplyBody {
    MsgAccepted = 0,
    MsgDenied = 1,
}

/// Authentication flavors.
#[derive(Debug, Clone, ToPrimitive, FromPrimitive)]
#[cfg_attr(test, derive(PartialEq))]
pub enum AuthFlavor {
    None = 0,
    Sys = 1,
    Short = 2,
    Dh = 3,
    RpcSecGss = 6,
}

#[derive(Debug, Clone)]
#[cfg_attr(test, derive(PartialEq))]
pub struct OpaqueAuth {
    pub flavor: AuthFlavor,
    pub body: Vec<u8>,
}

/// Parsed `AUTH_SYS` credential body (`authsys_parms`, RFC 5531 appendix A).
#[derive(Debug, Clone)]
#[cfg_attr(test, derive(PartialEq))]
pub struct AuthSysParams {
    /// Arbitrary ID the caller stamps on the credential; meaningful only to the caller.
    pub stamp: u32,
    /// Name of the caller's machine (at most [`AUTH_SYS_MAX_MACHINE_NAME`] bytes).
    pub machine_name: String,
    /// Effective user ID of the caller.
    pub uid: u32,
    /// Effective group ID of the caller.
    pub gid: u32,
    /// Auxiliary group IDs of the caller (at most [`AUTH_SYS_MAX_GIDS`] entries).
    pub gids: Vec<u32>,
}

/// Authenticated caller identity extracted from an RPC credential.
///
/// Only the flavors the server accepts are represented: `AUTH_NONE` (anonymous)
/// and `AUTH_SYS` (UNIX-style uid/gid). Any other flavor is rejected during
/// parsing with [`AuthStat::BadCred`].
#[derive(Debug, Clone)]
#[cfg_attr(test, derive(PartialEq))]
pub enum Credential {
    /// `AUTH_NONE`: anonymous caller, no identity provided.
    None,
    /// `AUTH_SYS`: UNIX-style caller identity.
    Sys(AuthSysParams),
}

pub enum RejectedReply {
    RpcMismatch = 0,
    AuthError = 1,
}

/// Represents a mismatch in program/protocol versions.
/// Returns highest and lowest versions of available versions of requested program
#[derive(Debug)]
pub struct VersionMismatch {
    pub low: u32,
    pub high: u32,
}

/// Errors that can occur during parsing.
#[derive(Debug)]
pub enum Error {
    /// The maximum element limit was exceeded.
    MaxElemLimit,
    /// An I/O error occurred.
    IO(io::Error),
    /// An enum discriminant mismatch occurred.
    EnumDiscMismatch,
    /// An incorrect string was encountered during UTF-8 conversion.
    IncorrectString(FromUtf8Error),
    /// An impossible type cast was attempted.
    ImpossibleTypeCast,
    /// A bad file handle was encountered.
    BadFileHandle,
    /// A message type mismatch occurred.
    MessageTypeMismatch,
    /// An RPC version mismatch occurred.
    RpcVersionMismatch(VersionMismatch),
    /// An authentication error occurred.
    Auth(AuthStat),
    /// A program mismatch occurred.
    ProgramMismatch,
    /// A procedure mismatch occurred.
    ProcedureMismatch,
    /// A program version mismatch occurred.
    ProgramVersionMismatch(VersionMismatch),
}
