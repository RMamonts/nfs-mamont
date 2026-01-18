//! Implements parsing for [`symlink::Args`] structure.

use std::io::Read;

use crate::parser::nfsv3::file;
use crate::parser::nfsv3::set_attr::new_attr;
use crate::parser::primitive::path;
use crate::parser::primitive::string;
use crate::parser::Result;
use crate::vfs::symlink;

pub fn args(src: &mut impl Read) -> Result<symlink::Args> {
    Ok(symlink::Args {
        dir: file::handle(src)?,
        name: string(src)?,
        attr: new_attr(src)?,
        path: path(src)?,
    })
}
