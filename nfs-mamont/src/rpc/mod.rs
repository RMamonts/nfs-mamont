use std::io;
use std::string::FromUtf8Error;

use num_derive::{FromPrimitive, ToPrimitive};

use auth::AuthStat;

#[cfg(feature = "arbitrary")]
use crate::consts::nfsv3::NFS_VERSION;
use crate::consts::rpc::MAX_AUTH_SIZE;

pub const RPC_VERSION: u32 = 2;


pub mod auth;

pub type Result<T> = core::result::Result<T, Error>;

pub enum AcceptStat {
    Success = 0,
    ProgUnavail = 1,
    ProgMismatch = 2,
    ProcUnavail = 3,
    GarbageArgs = 4,
    SystemErr = 5,
}

#[derive(ToPrimitive, FromPrimitive)]
pub enum RpcBody {
    Call = 0,
    Reply = 1,
}

pub enum ReplyBody {
    MsgAccepted = 0,
    MsgDenied = 1,
}

/// Authentication flavors.
#[derive(Debug, Clone, ToPrimitive, FromPrimitive)]
#[cfg_attr(test, derive(PartialEq))]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub enum AuthFlavor {
    None = 0,
    Sys = 1,
    Short = 2,
    Dh = 3,
    RpcSecGss = 6,
}

#[derive(Debug, Clone)]
#[cfg_attr(test, derive(PartialEq))]
pub struct OpaqueAuth {
    pub flavor: AuthFlavor,
    pub body: Vec<u8>,
}

#[cfg(feature = "arbitrary")]
impl arbitrary::Arbitrary<'_> for OpaqueAuth {
    fn arbitrary(u: &mut arbitrary::Unstructured<'_>) -> arbitrary::Result<Self> {
        let size = u.int_in_range(1..=MAX_AUTH_SIZE)?;
        let mut body = vec![0u8; size];
        u.fill_buffer(&mut body)?;
        Ok(Self { flavor: u.arbitrary::<AuthFlavor>()?, body })
    }
}
pub enum RejectedReply {
    RpcMismatch = 0,
    AuthError = 1,
}

/// Represents a mismatch in program/protocol versions.
/// Returns highest and lowest versions of available versions of requested program
#[derive(Debug)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct VersionMismatch {
    pub low: u32,
    pub high: u32,
}

/// Wrapper for [`Error`] to pass `xid` of procedure, this error
/// is associated with.
///
/// The semantically validated (header-level) errors flow out of the parser as
/// [`Error`], while structural parse failures are converted to that
/// type at the parser boundary.
#[derive(Debug)]
pub struct ErrorWrapper {
    pub xid: Option<u32>,
    pub error: Error,
}

/// Errors that can occur during parsing.
#[derive(Debug)]

pub enum Error {
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
    /// A message type mismatch occurred.
    MessageTypeMismatch,
    /// An RPC version mismatch occurred.
    RpcVersionMismatch(VersionMismatch),
    /// An authentication error occurred.
    Auth(AuthStat),
    /// A program mismatch occurred.
    ProgramMismatch,
    /// A procedure mismatch occurred.
    ProcedureMismatch,
    /// A program version mismatch occurred.
    ProgramVersionMismatch(VersionMismatch),
}

#[cfg(feature = "arbitrary")]
impl arbitrary::Arbitrary<'_> for Error {
    fn arbitrary(u: &mut arbitrary::Unstructured<'_>) -> arbitrary::Result<Self> {
        let idx = u.int_in_range(0..=11)?;
        Ok(match idx {
            0 => Error::EnumDiscMismatch,
            1 => Error::BadFileHandle,
            2 => Error::ImpossibleTypeCast,
            3 => Error::MaxElemLimit,
            4 => Error::MessageTypeMismatch,
            5 => Error::ProcedureMismatch,
            6 => Error::ProgramMismatch,
            7 => Error::IncorrectString(String::from_utf8(vec![0xFF]).unwrap_err()),
            8 => Error::Auth(AuthStat::BadCred),
            9 => Error::IO(io::Error::from_raw_os_error(22)),
            10 => {
                Error::RpcVersionMismatch(VersionMismatch { low: RPC_VERSION, high: RPC_VERSION })
            }
            11 => Error::ProgramVersionMismatch(VersionMismatch {
                low: NFS_VERSION,
                high: NFS_VERSION,
            }),
            _ => unreachable!(),
        })
    }
}
