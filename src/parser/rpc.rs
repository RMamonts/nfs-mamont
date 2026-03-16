use std::io::Read;

use crate::parser::primitive::{variant, vec_max_size};
use crate::parser::Result;
use crate::rpc::{AuthFlavor, OpaqueAuth, MAX_AUTH_SIZE};

#[derive(Debug)]
pub struct RpcMessage {
    pub program: u32,
    pub procedure: u32,
    pub version: u32,
}

pub fn auth(src: &mut impl Read) -> Result<OpaqueAuth> {
    Ok(OpaqueAuth { flavor: variant::<AuthFlavor>(src)?, body: vec_max_size(src, MAX_AUTH_SIZE)? })
}
