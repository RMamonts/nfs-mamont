use std::io::{Result, Write};

use crate::nfsv3::NFS3_CREATEVERFSIZE;
use crate::serializer::files::dir_op_arg;
use crate::serializer::{array, u32};
use crate::vfs::create::{Args, How, Verifier};

use super::set_attr::serialize_new_attr;

fn serialize_verifier(dest: &mut impl Write, verf: Verifier) -> Result<()> {
    array::<NFS3_CREATEVERFSIZE>(dest, verf.0)
}

fn serialize_how(dest: &mut impl Write, how: How) -> Result<()> {
    //TODO(change bare numbers to some enum)
    match how {
        How::Unchecked(new_attr) => u32(dest, 0).and_then(|_| serialize_new_attr(dest, new_attr)),
        How::Guarded(new_attr) => u32(dest, 1).and_then(|_| serialize_new_attr(dest, new_attr)),
        How::Exclusive(verifier) => u32(dest, 2).and_then(|_| serialize_verifier(dest, verifier)),
    }
}

pub fn create_args(dest: &mut impl Write, arg: Args) -> Result<()> {
    dir_op_arg(dest, arg.object).and_then(|_| serialize_how(dest, arg.how))
}
