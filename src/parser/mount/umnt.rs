use std::io::Read;

use crate::mount::umnt::UnmountArgs;
use crate::parser::nfsv3::file::file_path;
use crate::parser::Result;

/// Parses the arguments for an Unmount operation.
pub fn unmount(src: &mut impl Read) -> Result<UnmountArgs> {
    Ok(UnmountArgs(file_path(src)?))
}
