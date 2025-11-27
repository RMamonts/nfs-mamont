use std::io::Read;

use num_derive::FromPrimitive;

use crate::parser::primitive::{u8, variant, vec_max_size};
use crate::parser::Result;

const MAX_AUTH_SAZE: usize = 400;

#[derive(Debug, PartialOrd, PartialEq)]
pub enum AuthStat {
    AuthOk = 0,
    AuthBadCred = 1,
    AuthRejectedCred = 2,
    AuthBaDVerf = 3,
    AuthRejectedVerf = 4,
    AuthTooWeak = 5,
    AuthInvalidResp = 6,
    AuthFailed = 7,
    AuthKerbGeneric = 8,
    AuthTimeExpire = 9,
    AuthTktFile = 10,
    AuthDecode = 11,
    AuthNetAddr = 12,
    RpcSecGssCredProblem = 13,
    RpcSecGssCtxProblem = 14,
}

#[derive(FromPrimitive)]
pub enum AuthFlavor {
    AuthNone = 0,
    AuthSys = 1,
    AuthShort = 2,
    AuthDh = 3,
    RpcSecGss = 6,
}

struct OpaqueAuth {
    pub flavor: AuthFlavor,
    pub opaque: Vec<u8>,
}
pub fn auth(src: &mut dyn Read) -> Result<OpaqueAuth> {
    Ok(OpaqueAuth {
        flavor: variant::<AuthFlavor>(src)?,
        opaque: vec_max_size::<u8>(src, |s| u8(s), MAX_AUTH_SAZE)?,
    })
}
