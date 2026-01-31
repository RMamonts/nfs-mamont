#![allow(non_camel_case_types, clippy::upper_case_acronyms)]

use num_derive::ToPrimitive;

#[allow(dead_code)]
const RPC_VERSION: u32 = 2;
#[allow(dead_code)]
pub const MAX_AUTH_OPAQUE_LEN: usize = 400;

#[allow(dead_code)]
#[repr(u32)]
pub enum AcceptStat {
    Success = 0,
    ProgUnavail = 1,
    ProgMismatch = 2,
    ProcUnavail = 3,
    GarbageArgs = 4,
    SystemErr = 5,
}

#[allow(dead_code)]
#[derive(Debug, PartialEq, PartialOrd)]
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
    RpcSecCtsCredProblem = 14,
}

#[repr(u32)]
pub enum RpcBody {
    Call = 0,
    Reply = 1,
}

#[allow(dead_code)]
#[repr(u32)]
pub enum ReplyBody {
    MsgAccepted = 0,
    MsgDenied = 1,
}

#[allow(dead_code)]
#[derive(ToPrimitive)]
#[repr(u32)]
enum AuthFlavour {
    None = 0,
    Sys = 1,
    Short = 2,
    Dh = 3,
    RpcSecGss = 6,
}

#[allow(dead_code)]
pub struct OpaqueAuth {
    pub flavor: AuthFlavour,
    pub body: Vec<u8>,
}

#[allow(dead_code)]
#[repr(u32)]
pub enum RejectedReply {
    RPC_MISMATCH = 0,
    AUTH_ERROR = 1,
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct ProgramVersionMismatch {
    pub low: u32,
    pub high: u32,
}
#[allow(dead_code)]
#[derive(Debug)]
pub struct RPCVersionMismatch {
    pub low: u32,
    pub high: u32,
}
#[derive(Debug)]
pub enum Error {
    IncorrectPadding,
    ImpossibleTypeCast,
    BadFileHandle,
    MessageTypeMismatch,
    RpcVersionMismatch(RPCVersionMismatch),
    AuthError(AuthStat),
    ProgramMismatch,
    ProcedureMismatch,
    ProgramVersionMismatch(ProgramVersionMismatch),
}
