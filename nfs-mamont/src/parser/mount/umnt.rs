use std::io::Read;

use crate::consts::mount::MOUNT_DIRPATH_LEN;
use crate::mount::umnt::Args;
use crate::parser::primitive::string_max_size;
use crate::parser::Result;
use crate::rpc::Error;
use crate::vfs::file;

/// Parses the arguments for an Unmount operation.
pub fn unmount(src: &mut impl Read) -> Result<Args> {
    let path = string_max_size(src, MOUNT_DIRPATH_LEN)?;
    Ok(Args { dirpath: file::Path::new(path).map_err(Error::IO)? })
}
