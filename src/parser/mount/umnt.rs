use std::io::Read;

use crate::mount::umnt::UnmountArgs;
use crate::mount::MOUNT_DIRPATH_LEN;
use crate::parser::primitive::vec_max_size;
use crate::parser::Result;
use crate::rpc::Error;
use crate::vfs::file;

/// Parses the arguments for an Unmount operation.
pub fn unmount(src: &mut impl Read) -> Result<UnmountArgs> {
    let path = vec_max_size(src, MOUNT_DIRPATH_LEN)?;
    Ok(UnmountArgs(file::Path::from_utf8(path).map_err(Error::IO)?))
}
