use std::io::Read;

use crate::parser::primitive::{variant, vec_max_size};
use crate::parser::Result;
use crate::rpc::{AuthFlavour, OpaqueAuth};

const MAX_AUTH_SAZE: usize = 400;

#[derive(Debug)]
pub(super) struct RpcMessage {
    pub(super) program: u32,
    pub(super) procedure: u32,
    pub(super) version: u32,
}

pub fn auth(src: &mut impl Read) -> Result<OpaqueAuth> {
    Ok(OpaqueAuth { flavor: variant::<AuthFlavour>(src)?, body: vec_max_size(src, MAX_AUTH_SAZE)? })
}
