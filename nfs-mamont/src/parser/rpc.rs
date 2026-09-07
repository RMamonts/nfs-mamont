use std::io::Read;

use crate::parser::primitive::{string_max_size, u32, u32_as_usize, variant, vec_max_size};
use crate::parser::{Error, Result};
use crate::rpc::{
    AuthFlavor, AuthSysParams, Credential, OpaqueAuth, AUTH_SYS_MAX_GIDS,
    AUTH_SYS_MAX_MACHINE_NAME, MAX_AUTH_SIZE,
};

#[derive(Debug)]
pub struct RpcMessage {
    pub program: u32,
    pub procedure: u32,
    pub version: u32,
    pub cred: Credential,
}

/// Reads a raw `opaque_auth` (flavor + opaque body) from the stream.
pub fn auth(src: &mut impl Read) -> Result<OpaqueAuth> {
    Ok(OpaqueAuth { flavor: variant::<AuthFlavor>(src)?, body: vec_max_size(src, MAX_AUTH_SIZE)? })
}

/// Parses an `AUTH_SYS` credential body (`authsys_parms`, RFC 5531 appendix A):
///
/// ```text
/// struct authsys_parms {
///     unsigned int stamp;
///     string machinename<255>;
///     unsigned int uid;
///     unsigned int gid;
///     unsigned int gids<16>;
/// };
/// ```
///
/// `src` reads over the already-extracted opaque credential body, so it cannot
/// over-read into the rest of the RPC frame. Returns an error if the body is
/// truncated or violates the `machinename`/`gids` length limits.
pub fn authsys_parms(src: &mut impl Read) -> Result<AuthSysParams> {
    let stamp = u32(src)?;
    let machine_name = string_max_size(src, AUTH_SYS_MAX_MACHINE_NAME)?;
    let uid = u32(src)?;
    let gid = u32(src)?;

    let count = u32_as_usize(src)?;
    if count > AUTH_SYS_MAX_GIDS {
        return Err(Error::MaxElemLimit);
    }
    let mut gids = Vec::with_capacity(count);
    for _ in 0..count {
        gids.push(u32(src)?);
    }

    Ok(AuthSysParams { stamp, machine_name, uid, gid, gids })
}
