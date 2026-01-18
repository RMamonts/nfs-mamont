//! Implements parsing for [`mk_node::Args`] structure.

use std::io::Read;

use crate::parser::nfsv3::file;
use crate::parser::primitive::string;
use crate::parser::Result;
use crate::vfs::mk_node;

fn what(_src: &mut impl Read) -> Result<mk_node::What> {
    todo!()
}

pub fn args(src: &mut impl Read) -> Result<mk_node::Args> {
    Ok(mk_node::Args { dir: file::handle(src)?, name: string(src)?, what: what(src)? })
}
