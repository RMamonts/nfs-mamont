pub mod to_parse;

#[allow(unused)]
pub enum ParserError {
    StringTooLong,
    ReadError,
    EnumDiscMismatch,
    ArrayConvertError,
    IncorrectString,
    IncorrectFilehandle,
    IncorrectPadding,
}
