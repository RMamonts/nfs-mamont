use std::io::{Result, Write};

use crate::serializer::nfs::file_handle;
use crate::serializer::nfs::files::file_name;
use crate::serializer::{array, u32};
use crate::vfs::create::{Args, How, Verifier, VERIFY_LEN};

use super::set_attr::serialize_new_attr;

fn serialize_verifier(dest: &mut impl Write, verf: Verifier) -> Result<()> {
    array::<VERIFY_LEN>(dest, verf.0)
}

fn serialize_how(dest: &mut impl Write, how: How) -> Result<()> {
    match how {
        How::Unchecked(new_attr) => {
            u32(dest, How::Unchecked as u32).and_then(|_| serialize_new_attr(dest, new_attr))
        }
        How::Guarded(new_attr) => {
            u32(dest, How::Guarded as u32).and_then(|_| serialize_new_attr(dest, new_attr))
        }
        How::Exclusive(verifier) => {
            u32(dest, How::Exclusive as u32).and_then(|_| serialize_verifier(dest, verifier))
        }
    }
}

pub fn create_args(dest: &mut impl Write, arg: Args) -> Result<()> {
    file_handle(dest, arg.dir)
        .and_then(|_| file_name(dest, arg.name))
        .and_then(|_| serialize_how(dest, arg.how))
}
