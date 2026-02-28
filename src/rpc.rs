use num_derive::{FromPrimitive, ToPrimitive};

use crate::parser;

pub const RPC_VERSION: u32 = 2;

pub const MAX_AUTH_SIZE: usize = 400;

#[allow(dead_code)]
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

#[allow(dead_code)]
#[derive(ToPrimitive, FromPrimitive)]
pub enum RpcBody {
    Call = 0,
    Reply = 1,
}

#[allow(dead_code)]
pub enum ReplyBody {
    MsgAccepted = 0,
    MsgDenied = 1,
}

/// Authentication flavors.
#[derive(ToPrimitive, FromPrimitive)]
pub enum AuthFlavor {
    None = 0,
    Sys = 1,
    Short = 2,
    Dh = 3,
    RpcSecGss = 6,
}

#[allow(dead_code)]
pub struct OpaqueAuth {
    pub flavor: AuthFlavor,
    pub body: Vec<u8>,
}

#[allow(dead_code)]
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

pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur during parsing.
#[derive(Debug)]
pub enum Error {
    IncorrectMessage,
    ServerFailure(parser::Error),
    /// A message type mismatch occurred.
    MessageTypeMismatch,
    /// An RPC version mismatch occurred.
    RpcVersionMismatch(VersionMismatch),
    /// An authentication error occurred.
    AuthError(AuthStat),
    /// A program mismatch occurred.
    ProgramMismatch,
    /// A procedure mismatch occurred.
    ProcedureMismatch,
    /// A program version mismatch occurred.
    ProgramVersionMismatch(VersionMismatch),
}
