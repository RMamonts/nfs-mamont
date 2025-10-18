pub mod to_parse;

#[derive(Debug)]
#[allow(unused)]
pub enum Error {
    StringTooLong,
    VecTooLong,
    IOError,
    EnumDiscMismatch,
    ArrayConvertError,
    IncorrectString,
    IncorrectFilehandle,
    IncorrectPadding,
}
