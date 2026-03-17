use std::io::{Result, Write};

use crate::consts::nfsv3::NFS3_CREATEVERFSIZE;
use crate::serializer::files::dir_op_arg;
use crate::serializer::{array, u32};
use crate::vfs::create::{Args, How, HowMode, Verifier};

use crate::client::arguments::nfsv3::set_attr::serialize_new_attr;

/// Serializes [`Verifier`].
fn serialize_verifier(dest: &mut impl Write, verf: Verifier) -> Result<()> {
    array::<NFS3_CREATEVERFSIZE>(dest, verf.0)
}

/// Serializes [`How`].
fn serialize_how(dest: &mut impl Write, how: How) -> Result<()> {
    match how {
        How::Unchecked(new_attr) => {
            u32(dest, HowMode::Unchecked as u32).and_then(|_| serialize_new_attr(dest, new_attr))
        }
        How::Guarded(new_attr) => {
            u32(dest, HowMode::Guarded as u32).and_then(|_| serialize_new_attr(dest, new_attr))
        }
        How::Exclusive(verifier) => {
            u32(dest, HowMode::Exclusive as u32).and_then(|_| serialize_verifier(dest, verifier))
        }
    }
}

/// Serializes the arguments [`Args`] for an NFSv3 `CREATE` operation to the provided `Write` destination.
pub fn create_args(dest: &mut impl Write, arg: Args) -> Result<()> {
    dir_op_arg(dest, arg.object).and_then(|_| serialize_how(dest, arg.how))
}
