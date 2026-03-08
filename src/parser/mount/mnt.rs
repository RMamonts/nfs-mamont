use std::io::Read;

use crate::mount::mnt::MountArgs;
use crate::mount::MOUNT_DIRPATH_LEN;
use crate::parser::primitive::vec_max_size;
use crate::parser::Result;
use crate::rpc::Error;
use crate::vfs::file;

/// Parses the arguments for a Mount operation.
pub fn mount(src: &mut impl Read) -> Result<MountArgs> {
    let path = vec_max_size(src, MOUNT_DIRPATH_LEN)?;
    Ok(MountArgs(file::Path::from_utf8(path).map_err(Error::IO)?))
}
