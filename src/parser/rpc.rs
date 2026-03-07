use std::io::Read;

use crate::parser::primitive::{variant, vec_max_size};
use crate::parser::Result;
use crate::rpc::{AuthFlavor, OpaqueAuth, MAX_AUTH_SIZE};

#[derive(Debug, Copy, Clone)]
pub(super) struct RpcMessage {
    pub(super) xid: u32,
    pub(super) program: u32,
    pub(super) procedure: u32,
    pub(super) version: u32,
    pub(super) auth_flavor: AuthFlavor,
}

pub fn auth(src: &mut impl Read) -> Result<OpaqueAuth> {
    Ok(OpaqueAuth { flavor: variant::<AuthFlavor>(src)?, body: vec_max_size(src, MAX_AUTH_SIZE)? })
}
