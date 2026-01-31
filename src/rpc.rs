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

#[allow(dead_code)]
struct RpcMsg {
    xid: u32,
    body: RpcBody,
}

#[repr(u32)]
pub enum RpcBody {
    Call(CallBody) = 0,
    Reply(ReplyBody) = 1,
}

#[allow(dead_code)]
struct CallBody {
    rpcvers: u32,
    prog: u32,
    vers: u32,
    proc: u32,
    cred: opaque_auth,
    verf: opaque_auth,
}

#[allow(dead_code)]
#[repr(u32)]
pub enum ReplyBody {
    MsgAccepted(AcceptedReply) = 0,
    MsgDenied(RejectedReply) = 1,
}

#[allow(dead_code)]
struct mismatch_info {
    low: u32,
    high: u32,
}

#[allow(dead_code)]
struct AcceptedReply {
    verf: opaque_auth,
    reply_data: AcceptStat,
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
pub struct opaque_auth {
    pub flavor: AuthFlavour,
    pub body: Vec<u8>,
}

#[allow(dead_code)]
#[repr(u32)]
pub enum RejectedReply {
    RPC_MISMATCH(mismatch_info) = 0,
    AUTH_ERROR(AuthStat) = 1,
}
