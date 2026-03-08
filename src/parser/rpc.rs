use std::io::Read;

use crate::parser::primitive::{discard_opaque_max_size, variant};
use crate::parser::Result;
use crate::rpc::{AuthFlavor, MAX_AUTH_SIZE};

#[derive(Debug, Copy, Clone)]
pub(super) struct RpcMessage {
    pub(super) xid: u32,
    pub(super) program: u32,
    pub(super) procedure: u32,
    pub(super) version: u32,
    pub(super) auth_flavor: AuthFlavor,
}

pub fn auth_flavor(src: &mut impl Read) -> Result<AuthFlavor> {
    let flavor = variant::<AuthFlavor>(src)?;
    discard_opaque_max_size(src, MAX_AUTH_SIZE)?;
    Ok(flavor)
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use byteorder::{BigEndian, WriteBytesExt};

    use super::auth_flavor;
    use crate::parser::Error;
    use crate::rpc::AuthFlavor;

    #[test]
    fn auth_flavor_discards_body_and_padding() {
        let mut src = Vec::new();
        src.write_u32::<BigEndian>(AuthFlavor::Sys as u32).unwrap();
        src.write_u32::<BigEndian>(3).unwrap();
        src.extend_from_slice(&[1, 2, 3, 0]);

        let mut cursor = Cursor::new(src);
        let flavor = auth_flavor(&mut cursor).unwrap();

        assert_eq!(flavor, AuthFlavor::Sys);
        assert_eq!(cursor.position() as usize, cursor.get_ref().len());
    }

    #[test]
    fn auth_flavor_rejects_too_large_body() {
        let mut src = Vec::new();
        src.write_u32::<BigEndian>(AuthFlavor::None as u32).unwrap();
        src.write_u32::<BigEndian>((crate::rpc::MAX_AUTH_SIZE + 1) as u32).unwrap();

        let error = auth_flavor(&mut Cursor::new(src)).unwrap_err();
        assert!(matches!(error, Error::MaxElemLimit));
    }
}
