use std::io;
use std::string::FromUtf8Error;

pub mod nfsv3;
pub mod primitive;

#[derive(Debug)]
#[allow(unused)]
pub enum Error {
    MaxELemLimit,
    IO(io::Error),
    EnumDiscMismatch,
    IncorrectString(FromUtf8Error),
    IncorrectPadding,
}
