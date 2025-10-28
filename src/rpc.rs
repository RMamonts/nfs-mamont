#![allow(non_camel_case_types, clippy::upper_case_acronyms)]

#[allow(dead_code)]
pub const RPC_VERSION: u32 = 2;
#[allow(dead_code)]
const MAX_AUTH_OPAQUE_LEN: u32 = 400;

#[allow(dead_code)]
#[repr(u32)]
enum accept_stat {
    SUCCESS = 0,
    PROG_UNAVAIL = 1,
    PROG_MISMATCH(mismatch_info) = 2,
    PROC_UNAVAIL = 3,
    GARBAGE_ARGS = 4,
    SYSTEM_ERR = 5,
}

#[allow(dead_code)]
pub enum auth_stat {
    AUTH_OK = 0,
    AUTH_BADCRED = 1,
    AUTH_REJECTEDCRED = 2,
    AUTH_BADVERF = 3,
    AUTH_REJECTEDVERF = 4,
    AUTH_TOOWEAK = 5,
    AUTH_INVALIDRESP = 6,
    AUTH_FAILED = 7,
    AUTH_KERB_GENERIC = 8,
    AUTH_TIMEEXPIRE = 9,
    AUTH_TKT_FILE = 10,
    AUTH_DECODE = 11,
    AUTH_NET_ADDR = 12,
    RPCSEC_GSS_CREDPROBLEM = 13,
    RPCSEC_GSS_CTXPROBLEM = 14,
}

#[allow(dead_code)]
struct rpc_msg {
    xid: u32,
    body: rpc_body,
}

#[allow(dead_code)]
#[repr(u32)]
pub enum rpc_body {
    CALL(call_body) = 0,
    REPLY(reply_body) = 1,
}

#[allow(dead_code)]
pub struct call_body {
    rpcvers: u32,
    prog: u32,
    vers: u32,
    proc: u32,
    cred: opaque_auth,
    verf: opaque_auth,
}

#[allow(dead_code)]
#[repr(u32)]
pub enum reply_body {
    MSG_ACCEPTED(accepted_reply) = 0,
    MSG_DENIED(rejected_reply) = 1,
}

#[allow(dead_code)]
pub struct mismatch_info {
    low: u32,
    high: u32,
}

#[allow(dead_code)]
pub struct accepted_reply {
    verf: opaque_auth,
    reply_data: accept_stat,
}

#[allow(dead_code)]
#[repr(u32)]
enum auth_flavor {
    AUTH_NONE = 0,
    AUTH_SYS = 1,
    AUTH_SHORT = 2,
    AUTH_DH = 3,
    RPCSEC_GSS = 6,
}

#[allow(dead_code)]
struct opaque_auth {
    flavor: auth_flavor,
    body: Vec<u8>,
}

#[allow(dead_code)]
#[repr(u32)]
pub enum rejected_reply {
    RPC_MISMATCH(mismatch_info) = 0,
    AUTH_ERROR(auth_stat) = 1,
}
