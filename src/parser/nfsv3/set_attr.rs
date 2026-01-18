//! Implements parsing for [`set_attr::Args`] structure.

use std::io::Read;

use crate::parser::nfsv3::file;
use crate::parser::Result;
use crate::vfs::set_attr;

pub fn new_attr(_src: &mut impl Read) -> Result<set_attr::NewAttr> {
    todo!()
}

pub fn guard(_src: &mut impl Read) -> Result<Option<set_attr::Guard>> {
    todo!()
}

pub fn args(src: &mut impl Read) -> Result<set_attr::Args> {
    Ok(set_attr::Args { file: file::handle(src)?, new_attr: new_attr(src)?, guard: guard(src)? })
}
