use std::io::Read;

use crate::mount::mnt::MountArgs;
use crate::parser::nfsv3::file::file_path;
use crate::parser::Result;

/// Parses the arguments for a Mount operation.
pub fn mount(src: &mut impl Read) -> Result<MountArgs> {
    Ok(MountArgs(file_path(src)?))
}
