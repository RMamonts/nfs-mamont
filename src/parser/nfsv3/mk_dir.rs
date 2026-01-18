//! Implements parsing for [`mk_dir::Args`] structure.

use std::io::Read;

use crate::parser::nfsv3::file;
use crate::parser::primitive::string;
use crate::parser::Result;
use crate::vfs::mk_dir;

fn new_attr(_src: &mut impl Read) -> Result<crate::vfs::set_attr::NewAttr> {
    todo!()
}

pub fn args(src: &mut impl Read) -> Result<mk_dir::Args> {
    Ok(mk_dir::Args { dir: file::handle(src)?, name: string(src)?, attr: new_attr(src)? })
}
