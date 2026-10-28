//! Parser-level (structural) errors.
//!
//! These are the errors produced while *structurally* decoding XDR-encoded
//! data: I/O failures, enum/limit violations and invalid opaque fields.

use std::io;
use std::string::FromUtf8Error;

/// Errors that can occur during structural parsing of RPC-related data.
#[derive(Debug)]
pub enum Error {
    /// Failed to pase data
    InvalidData,
    /// Parsed successfully, but failed to pass internal consistency checks for type
    FormatMismatch,
    /// The maximum element limit was exceeded.
    MaxElemLimit,
    /// An I/O error occurred.
    IO(io::Error),
    /// An enum discriminant mismatch occurred.
    EnumDiscMismatch,
    /// An incorrect string was encountered during UTF-8 conversion.
    IncorrectString(FromUtf8Error),
    /// An impossible type cast was attempted.
    ImpossibleTypeCast,
    /// A bad file handle was encountered.
    BadFileHandle,
}
