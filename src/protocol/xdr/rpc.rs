//! This module provides Sun Remote Procedure Call (RPC).
//!
//! <https://datatracker.ietf.org/doc/html/rfc5531>

use std::io::{Read, Write};

use num_derive::{FromPrimitive, ToPrimitive};

use super::{
    deserialize, Deserialize, DeserializeEnum, DeserializeStruct, Serialize, SerializeEnum,
    SerializeStruct,
};

// TODO
// - check bounds of opaque data.
// - rewrite Deserialize to work without Default for Copy types.

pub const PROTOCOL_VERSION: u32 = 2;

/// Authentication flavor (mechanism) identifiers for RPC.
#[allow(non_camel_case_types)]
#[derive(Copy, Clone, Debug, FromPrimitive, ToPrimitive)]
pub enum auth_flavor {
    AUTH_NONE = 0,
    AUTH_SYS = 1,
    AUTH_SHORT = 2,
    AUTH_DH = 3,
    RPCSEC_GSS = 6,
}
impl SerializeEnum for auth_flavor {}
impl DeserializeEnum for auth_flavor {}

/// Authentication data structure used in RPC protocol for both
/// client and server authentication.
#[allow(non_camel_case_types)]
#[derive(Clone, Debug)]
pub struct opaque_auth {
    /// The authentication mechanism tag being used.
    pub flavor: auth_flavor,
    /// The opaque authentication data associated with that mechanism.
    pub body: Vec<u8>,
}
DeserializeStruct!(opaque_auth, flavor, body);
SerializeStruct!(opaque_auth, flavor, body);

impl Default for opaque_auth {
    fn default() -> opaque_auth {
        opaque_auth { flavor: auth_flavor::AUTH_NONE, body: Vec::new() }
    }
}

#[allow(non_camel_case_types)]
#[derive(Clone, Debug, Default)]
/// UNIX-style credentials used for authentication
pub struct auth_unix {
    /// Timestamp to prevent replay attacks
    pub stamp: u32,
    /// The name of the client machine
    pub machinename: Vec<u8>,
    /// The effective user ID of the caller
    pub uid: u32,
    /// The effective group ID of the caller
    pub gid: u32,
    /// A list of additional group IDs for the caller
    pub gids: Vec<u32>,
}
DeserializeStruct!(auth_unix, stamp, machinename, uid, gid, gids);
SerializeStruct!(auth_unix, stamp, machinename, uid, gid, gids);

/// Type of RPC message.
#[allow(non_camel_case_types, clippy::upper_case_acronyms)]
#[derive(Clone, Copy, Debug, ToPrimitive, FromPrimitive)]
enum msg_type {
    CALL = 0,
    REPLY = 1,
}
impl SerializeEnum for msg_type {}
impl DeserializeEnum for msg_type {}

/// Forms of RPC replay.
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, ToPrimitive, FromPrimitive)]
enum reply_stat {
    MSG_ACCEPTED = 0,
    MSG_DENIED = 1,
}
impl SerializeEnum for reply_stat {}
impl DeserializeEnum for reply_stat {}

/// Status of an attempt to call a remote procedure.
#[allow(non_camel_case_types, clippy::upper_case_acronyms)]
#[derive(Clone, Copy, Debug, FromPrimitive, ToPrimitive)]
enum accept_stat {
    /// RPC executed successfully.
    SUCCESS = 0,
    /// Remote hasn't exported program.
    PROG_UNAVAIL = 1,
    /// Remote can't support version.
    PROG_MISMATTCH = 2,
    /// Program can't support procedure.
    PROC_UNAVAIL = 3,
    /// Procedure can't decode params.
    GARBAGE_ARGS = 4,
    /// System error, e. g. memory allocation failure.
    SYSTEM_ERR = 5,
}
impl SerializeEnum for accept_stat {}
impl DeserializeEnum for accept_stat {}

/// Reasons why a call message was rejected.
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, ToPrimitive, FromPrimitive)]
enum reject_stat {
    /// RPC version mismatch.
    RPC_MISMATCH = 0,
    /// Remote can't authenticate caller.
    AUTH_ERROR = 1,
}
impl SerializeEnum for reject_stat {}
impl DeserializeEnum for reject_stat {}

/// Status codes indicating why authentication failed.
#[allow(non_camel_case_types)]
#[derive(Copy, Clone, Debug, FromPrimitive, ToPrimitive, Default)]
pub enum auth_stat {
    /// Success.
    #[default]
    AUTH_OK = 0,
    /// Remote side failure. Bad credential (seal broken).
    AUTH_BADCRED = 1,
    /// Remote side failure. Client must begin new session.
    AUTH_REJECTEDCRED = 2,
    /// Remote side failure. Vad verifier (seal broken).
    AUTH_BADVERF = 3,
    /// Remote side failure. Verifier expired or replayed.
    AUTH_REJECTEDVERF = 4,
    /// Remote side failure. Rejected for security reasons.
    AUTH_TOOWEAK = 5,
    /// Failed locally. Bogus response verifier.
    AUTH_INVALIDRESP = 6,
    /// Failed locally. Reson unkown.
    AUTH_FAILED = 7,
    /// Kerberos generic error. AUTH_KERB errors; deprecated. See [RFC2695].
    AUTH_KERB_GENERIC = 8,
    /// Time of credential expired. AUTH_KERB errors; deprecated. See [RFC2695].
    AUTH_TIMEEXPIRE = 9,
    /// Problem with ticket file. AUTH_KERB errors; deprecated. See [RFC2695].
    AUTH_TKT_FILE = 10,
    /// Can't decode authenticator. AUTH_KERB errors; deprecated. See [RFC2695].
    AUTH_DECODE = 11,
    /// Wrong net address in ticket. AUTH_KERB errors; deprecated. See [RFC2695].
    AUTH_NET_ADDR = 12,
    /// RPCSEC_GSS GSS related errors. No credentials for user.
    RPCSEC_GSS_CREDPROBLEM = 13,
    /// RPCSEC_GSS GSS related errors. Problem with context.
    RPCSEC_GSS_CTXPROBLEM = 14,
}
impl SerializeEnum for auth_stat {}
impl DeserializeEnum for auth_stat {}

/// RPC message.
#[allow(non_camel_case_types)]
#[derive(Clone, Debug)]
pub struct rpc_msg {
    /// Transaction identifier used to match calls and replies.
    pub xid: u32,
    /// The body of the RPC message (call or reply).
    pub body: rpc_body,
}
DeserializeStruct!(rpc_msg, xid, body);
SerializeStruct!(rpc_msg, xid, body);

/// The body of an RPC message, which can be either a call or a reply
#[allow(non_camel_case_types)]
#[allow(clippy::upper_case_acronyms)]
#[derive(Clone, Debug)]
pub enum rpc_body {
    /// A call to a remote procedure
    CALL(call_body),
    /// A reply from a remote procedure
    REPLY(reply_body),
}

impl Serialize for rpc_body {
    fn serialize<R: Write>(&self, dest: &mut R) -> std::io::Result<()> {
        match self {
            rpc_body::CALL(v) => {
                msg_type::CALL.serialize(dest)?;
                v.serialize(dest)?;
            }
            rpc_body::REPLY(v) => {
                msg_type::REPLY.serialize(dest)?;
                v.serialize(dest)?;
            }
        }
        Ok(())
    }
}
impl Deserialize for rpc_body {
    fn deserialize<R: Read>(src: &mut R) -> std::io::Result<Self> {
        match deserialize::<msg_type>(src)? {
            msg_type::CALL => Ok(rpc_body::CALL(deserialize(src)?)),
            msg_type::REPLY => Ok(rpc_body::REPLY(deserialize(src)?)),
        }
    }
}

/// The body of an RPC call.
#[allow(non_camel_case_types)]
#[derive(Clone, Debug)]
pub struct call_body {
    /// RPC version, must be equal to two (2).
    pub rpcvers: u32,
    /// The program to call.
    pub prog: u32,
    /// The version of the program.
    pub vers: u32,
    /// The procedure within the program to call.
    pub proc: u32,
    /// Authentication credentials for the caller.
    pub cred: opaque_auth,
    /// Authentication verifier for the caller.
    pub verf: opaque_auth,
    // procedure-specific parameters start here.
}
DeserializeStruct!(call_body, rpcvers, prog, vers, proc, cred, verf);
SerializeStruct!(call_body, rpcvers, prog, vers, proc, cred, verf);

/// The body of an RPC reply.
#[allow(non_camel_case_types)]
#[derive(Clone, Debug)]
pub enum reply_body {
    /// The call was accepted.
    MSG_ACCEPTED(accepted_reply),
    /// The call was denied.
    MSG_DENIED(rejected_reply),
}

impl Serialize for reply_body {
    fn serialize<R: Write>(&self, dest: &mut R) -> std::io::Result<()> {
        match self {
            reply_body::MSG_ACCEPTED(v) => {
                reply_stat::MSG_ACCEPTED.serialize(dest)?;
                v.serialize(dest)?;
            }
            reply_body::MSG_DENIED(v) => {
                reply_stat::MSG_DENIED.serialize(dest)?;
                v.serialize(dest)?;
            }
        }
        Ok(())
    }
}

impl Deserialize for reply_body {
    fn deserialize<R: Read>(src: &mut R) -> std::io::Result<Self> {
        match deserialize::<reply_stat>(src)? {
            reply_stat::MSG_ACCEPTED => Ok(reply_body::MSG_ACCEPTED(deserialize(src)?)),
            reply_stat::MSG_DENIED => Ok(reply_body::MSG_DENIED(deserialize(src)?)),
        }
    }
}

/// Information about program version mismatch
#[allow(non_camel_case_types)]
#[derive(Clone, Debug)]
pub struct mismatch_info {
    /// Lowest version supported
    pub low: u32,
    /// Highest version supported
    pub high: u32,
}
DeserializeStruct!(mismatch_info, low, high);
SerializeStruct!(mismatch_info, low, high);
/// Reply to an RPC call that was accepted by the server.
///
/// Even though the call was accepted, there could still be an error in processing it.
/// The structure contains:
/// - An authentication verifier generated by the server to validate itself to the client
/// - A union containing the actual reply data, discriminated by `accept_stat` enum
///
/// The `reply_data` union has the following arms:
/// - `SUCCESS`: Contains protocol-specific success response
/// - `PROG_UNAVAIL`: Program not available (void)
/// - `PROG_MISMATCH`: Program version mismatch, includes supported version range
/// - `PROC_UNAVAIL`: Procedure not available (void)
/// - `GARBAGE_ARGS`: Arguments could not be decoded (void)
#[allow(non_camel_case_types)]
#[derive(Clone, Debug, Default)]
pub struct accepted_reply {
    /// Authentication verifier from server
    pub verf: opaque_auth,
    /// Reply data union discriminated by `accept_stat`
    pub reply_data: accept_body,
}
DeserializeStruct!(accepted_reply, verf, reply_data);
SerializeStruct!(accepted_reply, verf, reply_data);

/// Response data for an accepted RPC call, discriminated by `accept_stat`.
///
/// This enum represents the possible outcomes of an accepted RPC call:
/// - `SUCCESS`: Call completed successfully, response data is protocol-specific
/// - `PROG_UNAVAIL`: The requested program is not available on this server
/// - `PROG_MISMATCH`: Program version mismatch, includes supported version range
/// - `PROC_UNAVAIL`: The requested procedure is not available in this program
/// - `GARBAGE_ARGS`: The server could not decode the call arguments
#[allow(non_camel_case_types)]
#[allow(clippy::upper_case_acronyms)]
#[derive(Clone, Debug, Default)]
#[repr(u32)]
pub enum accept_body {
    /// Call completed successfully
    #[default]
    SUCCESS,
    /// Program is not available on this server
    PROG_UNAVAIL,
    /// Program version mismatch, includes supported version range
    PROG_MISMATCH(mismatch_info),
    /// Requested procedure is not available
    PROC_UNAVAIL,
    /// Server could not decode the call arguments
    GARBAGE_ARGS,
}

impl Serialize for accept_body {
    fn serialize<R: Write>(&self, dest: &mut R) -> std::io::Result<()> {
        match self {
            accept_body::SUCCESS => {
                accept_stat::SUCCESS.serialize(dest)?;
            }
            accept_body::PROG_UNAVAIL => {
                accept_stat::PROG_UNAVAIL.serialize(dest)?;
            }
            accept_body::PROG_MISMATCH(v) => {
                accept_stat::PROG_MISMATTCH.serialize(dest)?;
                v.serialize(dest)?;
            }
            accept_body::PROC_UNAVAIL => {
                accept_stat::PROC_UNAVAIL.serialize(dest)?;
            }
            accept_body::GARBAGE_ARGS => {
                accept_stat::GARBAGE_ARGS.serialize(dest)?;
            }
        }

        Ok(())
    }
}
impl Deserialize for accept_body {
    fn deserialize<R: Read>(src: &mut R) -> std::io::Result<Self> {
        Ok(match deserialize::<u32>(src)? {
            0 => accept_body::SUCCESS,
            1 => accept_body::PROG_UNAVAIL,
            2 => accept_body::PROG_MISMATCH(deserialize(src)?),
            3 => accept_body::PROC_UNAVAIL,
            4 => accept_body::GARBAGE_ARGS,
            accept_stat => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Invalid accept stat in accept_body: {accept_stat}"),
                ));
            }
        })
    }
}

/// Reply sent when an RPC call is rejected by the server.
///
/// The call can be rejected for two reasons:
/// 1. RPC Version Mismatch (`RPC_MISMATCH`):
///    - Server is not running a compatible version of the RPC protocol
///    - Server returns the lowest and highest supported RPC versions
///
/// 2. Authentication Error (`AUTH_ERROR`):
///    - Server refuses to authenticate the caller
///    - Returns specific auth failure status code
///
/// The discriminant for this enum is `reject_stat` which indicates the
/// rejection reason.
#[allow(non_camel_case_types)]
#[derive(Clone, Debug)]
pub enum rejected_reply {
    /// RPC version mismatch - includes supported version range
    RPC_MISMATCH(mismatch_info),
    /// Authentication failed - includes specific error code
    AUTH_ERROR(auth_stat),
}

impl Default for rejected_reply {
    fn default() -> rejected_reply {
        rejected_reply::AUTH_ERROR(auth_stat::default())
    }
}

impl Serialize for rejected_reply {
    fn serialize<R: Write>(&self, dest: &mut R) -> std::io::Result<()> {
        match self {
            rejected_reply::RPC_MISMATCH(v) => {
                reject_stat::RPC_MISMATCH.serialize(dest)?;
                v.serialize(dest)?;
            }
            rejected_reply::AUTH_ERROR(v) => {
                reject_stat::AUTH_ERROR.serialize(dest)?;
                (*v as u32).serialize(dest)?;
            }
        }

        Ok(())
    }
}
impl Deserialize for rejected_reply {
    fn deserialize<R: Read>(src: &mut R) -> std::io::Result<Self> {
        Ok(match deserialize::<u32>(src)? {
            0 => rejected_reply::RPC_MISMATCH(deserialize(src)?),
            1 => rejected_reply::AUTH_ERROR(deserialize(src)?),
            stat => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Invalid reject stat in rejected_reply: {stat}"),
                ))
            }
        })
    }
}

/// Creates a reply message indicating that the arguments could not be decoded
pub fn garbage_args_reply_message(xid: u32) -> rpc_msg {
    let reply = reply_body::MSG_ACCEPTED(accepted_reply {
        verf: opaque_auth::default(),
        reply_data: accept_body::GARBAGE_ARGS,
    });
    rpc_msg { xid, body: rpc_body::REPLY(reply) }
}

/// Creates a successful reply message with no additional data
pub fn make_success_reply(xid: u32) -> rpc_msg {
    let reply = reply_body::MSG_ACCEPTED(accepted_reply {
        verf: opaque_auth::default(),
        reply_data: accept_body::SUCCESS,
    });
    rpc_msg { xid, body: rpc_body::REPLY(reply) }
}
