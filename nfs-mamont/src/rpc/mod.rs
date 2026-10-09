use std::io;
use std::string::FromUtf8Error;

use num_derive::{FromPrimitive, ToPrimitive};

use auth::AuthStat;

use crate::vfs;

pub mod auth;

pub enum AcceptStat {
    Success = 0,
    ProgUnavail = 1,
    ProgMismatch = 2,
    ProcUnavail = 3,
    GarbageArgs = 4,
    SystemErr = 5,
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
    /// A file name or path is longer than the server supports.
    NameTooLong,
    /// MKNOD was asked to create a regular file, a directory or a symbolic link.
    BadType,
    /// The call is malformed in a way no other variant describes, for example
    /// its arguments end early or are followed by unparsed bytes.
    Malformed(&'static str),
    /// The NFSv3 call is well-formed but fails with `status` before reaching a
    /// backend, for example because its file handle has a foreign size.
    Nfs3Status {
        /// Procedure whose failed result is sent.
        procedure: u32,
        /// Status of the failed result.
        status: vfs::Error,
    },
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
