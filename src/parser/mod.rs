pub mod to_parse;

#[derive(Debug)]
#[allow(unused)]
pub enum Error {
    StringTooLong,
    VecTooLong,
    IO,
    EnumDiscMismatch,
    ArrayConvert,
    IncorrectString,
    IncorrectFilehandle,
    IncorrectPadding,
}
