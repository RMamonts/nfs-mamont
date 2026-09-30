//! Implements parsing for [`mk_dir::Args`] structure.

use std::io::Read;

use crate::parser::nfsv3::create::new_attr;
use crate::parser::nfsv3::file;
use crate::parser::nfsv3::file::file_name;
use crate::parser::Result;
use crate::vfs::mk_dir;

/// Parses the arguments for an NFSv3 `MKDIR` operation from the provided `Read` source.
pub fn args(src: &mut impl Read) -> Result<mk_dir::Args> {
    Ok(mk_dir::Args {
        object: crate::vfs::DirOpArgs { dir: file::handle(src)?, name: file_name(src)? },
        attr: new_attr(src)?,
    })
}
