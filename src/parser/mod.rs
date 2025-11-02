pub mod mount;
pub mod nfsv3;
pub mod primitive;
mod tests;

use std::io;
use std::string::FromUtf8Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
#[allow(unused)]
pub enum Error {
    MaxELemLimit,
    IO(io::Error),
    EnumDiscMismatch,
    IncorrectString(FromUtf8Error),
    IncorrectPadding,
    ImpossibleTypeCast,
    BadFileHandle,
}
