//! MOUNT v3 `MNT` request and reply (RFC 1813, section 5).

use nfs_mamont::consts::mount::{MOUNT_MNT, MOUNT_PROGRAM, MOUNT_VERSION};

use super::xdr::{put_str, Reader};
use super::{Connection, Error};

/// `dirpath` argument of `MNT`.
pub fn args(path: &str) -> Vec<u8> {
    let mut buf = Vec::new();
    put_str(&mut buf, path);
    buf
}

/// `mountres3_ok`.
#[derive(Clone, Debug)]
pub struct Mount {
    /// Root handle of the export.
    pub handle: Vec<u8>,
    /// Authentication flavors accepted for the export.
    pub auth_flavors: Vec<u32>,
}

/// Decodes `mountres3`.
pub fn parse(body: &[u8]) -> Result<Mount, Error> {
    let mut reader = Reader::new(body);
    match reader.u32()? {
        0 => {}
        code => return Err(Error::Status(code)),
    }
    let handle = reader.opaque()?.to_vec();
    let count = reader.u32()?;
    let mut auth_flavors = Vec::with_capacity(count.min(16) as usize);
    for _ in 0..count {
        auth_flavors.push(reader.u32()?);
    }

    Ok(Mount { handle, auth_flavors })
}

/// Mounts `path` and returns its root handle.
pub async fn mount(conn: &mut Connection, path: &str) -> Result<Vec<u8>, Error> {
    let body = conn.call(MOUNT_PROGRAM, MOUNT_VERSION, MOUNT_MNT, &args(path)).await?;

    Ok(parse(body)?.handle)
}
