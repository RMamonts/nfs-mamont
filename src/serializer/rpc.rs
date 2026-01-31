use crate::rpc::{OpaqueAuth, MAX_AUTH_OPAQUE_LEN};
use crate::serializer::{u32, vec_max_size};
use num_traits::ToPrimitive;
use std::io;
use std::io::Write;

pub fn auth_opaque(dest: &mut dyn Write, data: OpaqueAuth) -> io::Result<()> {
    let n = data
        .flavor
        .to_u32()
        .ok_or(io::Error::new(io::ErrorKind::InvalidInput, "invalid flavor"))?;
    u32(dest, n)?;
    vec_max_size(dest, data.body, MAX_AUTH_OPAQUE_LEN)
}
