pub mod to_parse;

#[derive(Debug)]
#[allow(unused)]
pub enum ParserError {
    StringTooLong,
    VecTooLong,
    ReadError,
    EnumDiscMismatch,
    ArrayConvertError,
    IncorrectString,
    IncorrectFilehandle,
    IncorrectPadding,
}
