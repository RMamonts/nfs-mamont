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
#[derive(Copy, Clone, Debug, FromPrimitive, ToPrimitive)]
pub enum AuthFlavor {
    AuthNone = 0,
    AuthSys = 1,
    AuthShort = 2,
    AuthDH = 3,
    RPCSesGSS = 6,
}
impl SerializeEnum for AuthFlavor {}
impl DeserializeEnum for AuthFlavor {}

/// Authentication data structure used in RPC protocol for both
/// client and server authentication.
#[derive(Clone, Debug)]
pub struct OpaqueAuth {
    /// The authentication mechanism tag being used.
    pub flavor: AuthFlavor,
    /// The opaque authentication data associated with that mechanism.
    pub body: Vec<u8>,
}
DeserializeStruct!(OpaqueAuth, flavor, body);
SerializeStruct!(OpaqueAuth, flavor, body);

impl Default for OpaqueAuth {
    fn default() -> OpaqueAuth {
        OpaqueAuth { flavor: AuthFlavor::AuthNone, body: Vec::new() }
    }
}

#[derive(Clone, Debug, Default)]
/// UNIX-style credentials used for authentication
pub struct AuthUnix {
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
DeserializeStruct!(AuthUnix, stamp, machinename, uid, gid, gids);
SerializeStruct!(AuthUnix, stamp, machinename, uid, gid, gids);

/// Type of RPC message.
#[allow(non_camel_case_types, clippy::upper_case_acronyms)]
#[derive(Clone, Copy, Debug, ToPrimitive, FromPrimitive)]
enum MsgType {
    Call = 0,
    Reply = 1,
}
impl SerializeEnum for MsgType {}
impl DeserializeEnum for MsgType {}

/// Forms of RPC replay.
#[derive(Clone, Copy, Debug, ToPrimitive, FromPrimitive)]
enum ReplyStat {
    MsgAccepted = 0,
    MsgDenied = 1,
}
impl SerializeEnum for ReplyStat {}
impl DeserializeEnum for ReplyStat {}

/// Status of an attempt to call a remote procedure.
#[allow(non_camel_case_types, clippy::upper_case_acronyms)]
#[derive(Clone, Copy, Debug, FromPrimitive, ToPrimitive)]
enum AcceptStat {
    /// RPC executed successfully.
    Success = 0,
    /// Remote hasn't exported program.
    ProgUnavail = 1,
    /// Remote can't support version.
    ProgMismatch = 2,
    /// Program can't support procedure.
    ProcUnavail = 3,
    /// Procedure can't decode params.
    GarbageAargs = 4,
    /// System error, e. g. memory allocation failure.
    SYSTEM_ERR = 5,
}
impl SerializeEnum for AcceptStat {}
impl DeserializeEnum for AcceptStat {}

/// Reasons why a call message was rejected.
#[derive(Clone, Copy, Debug, ToPrimitive, FromPrimitive)]
enum RejectStat {
    /// RPC version mismatch.
    RPCMismatch = 0,
    /// Remote can't authenticate caller.
    AuthError = 1,
}
impl SerializeEnum for RejectStat {}
impl DeserializeEnum for RejectStat {}

/// Status codes indicating why authentication failed.
#[derive(Copy, Clone, Debug, FromPrimitive, ToPrimitive, Default)]
pub enum AuthStat {
    /// Success.
    #[default]
    AuthOk = 0,
    /// Remote side failure. Bad credential (seal broken).
    AuthBabcred = 1,
    /// Remote side failure. Client must begin new session.
    AuthRjectedCred = 2,
    /// Remote side failure. Vad verifier (seal broken).
    AuthBadVerf = 3,
    /// Remote side failure. Verifier expired or replayed.
    AuthRectedVerf = 4,
    /// Remote side failure. Rejected for security reasons.
    AuthTooWeak = 5,
    /// Failed locally. Bogus response verifier.
    AuthInvalidResp = 6,
    /// Failed locally. Reson unkown.
    AuthFailed = 7,
    /// Kerberos generic error. AUTH_KERB errors; deprecated. See [RFC2695].
    AuthKerbGeneric = 8,
    /// Time of credential expired. AUTH_KERB errors; deprecated. See [RFC2695].
    AuthTimeExpire = 9,
    /// Problem with ticket file. AUTH_KERB errors; deprecated. See [RFC2695].
    AuthTktFile = 10,
    /// Can't decode authenticator. AUTH_KERB errors; deprecated. See [RFC2695].
    AuthDecode = 11,
    /// Wrong net address in ticket. AUTH_KERB errors; deprecated. See [RFC2695].
    AuthNetAddr = 12,
    /// RPCSEC_GSS GSS related errors. No credentials for user.
    RPCSecGSSCredProblem = 13,
    /// RPCSEC_GSS GSS related errors. Problem with context.
    RPCSesGSSCtxProblem = 14,
}
impl SerializeEnum for AuthStat {}
impl DeserializeEnum for AuthStat {}

/// RPC message.
#[derive(Clone, Debug)]
pub struct RPCMsg {
    /// Transaction identifier used to match calls and replies.
    pub xid: u32,
    /// The body of the RPC message (call or reply).
    pub body: RPCBody,
}
DeserializeStruct!(RPCMsg, xid, body);
SerializeStruct!(RPCMsg, xid, body);

/// The body of an RPC message, which can be either a call or a reply
#[derive(Clone, Debug)]
pub enum RPCBody {
    /// A call to a remote procedure
    Call(CallBody),
    /// A reply from a remote procedure
    Reply(ReplyBody),
}

impl Serialize for RPCBody {
    fn serialize<R: Write>(&self, dest: &mut R) -> std::io::Result<()> {
        match self {
            RPCBody::Call(v) => {
                MsgType::Call.serialize(dest)?;
                v.serialize(dest)?;
            }
            RPCBody::Reply(v) => {
                MsgType::Reply.serialize(dest)?;
                v.serialize(dest)?;
            }
        }
        Ok(())
    }
}
impl Deserialize for RPCBody {
    fn deserialize<R: Read>(src: &mut R) -> std::io::Result<Self> {
        match deserialize::<MsgType>(src)? {
            MsgType::Call => Ok(RPCBody::Call(deserialize(src)?)),
            MsgType::Reply => Ok(RPCBody::Reply(deserialize(src)?)),
        }
    }
}

/// The body of an RPC call.
#[derive(Clone, Debug)]
pub struct CallBody {
    /// RPC version, must be equal to two (2).
    pub rpcvers: u32,
    /// The program to call.
    pub prog: u32,
    /// The version of the program.
    pub vers: u32,
    /// The procedure within the program to call.
    pub proc: u32,
    /// Authentication credentials for the caller.
    pub cred: OpaqueAuth,
    /// Authentication verifier for the caller.
    pub verf: OpaqueAuth,
    // procedure-specific parameters start here.
}
DeserializeStruct!(CallBody, rpcvers, prog, vers, proc, cred, verf);
SerializeStruct!(CallBody, rpcvers, prog, vers, proc, cred, verf);

/// The body of an RPC reply.
#[derive(Clone, Debug)]
pub enum ReplyBody {
    /// The call was accepted.
    MsgAccepted(AcceptedReply),
    /// The call was denied.
    MsgDenied(RejectedReply),
}

impl Serialize for ReplyBody {
    fn serialize<R: Write>(&self, dest: &mut R) -> std::io::Result<()> {
        match self {
            ReplyBody::MsgAccepted(v) => {
                ReplyStat::MsgAccepted.serialize(dest)?;
                v.serialize(dest)?;
            }
            ReplyBody::MsgDenied(v) => {
                ReplyStat::MsgDenied.serialize(dest)?;
                v.serialize(dest)?;
            }
        }
        Ok(())
    }
}

impl Deserialize for ReplyBody {
    fn deserialize<R: Read>(src: &mut R) -> std::io::Result<Self> {
        match deserialize::<ReplyStat>(src)? {
            ReplyStat::MsgAccepted => Ok(ReplyBody::MsgAccepted(deserialize(src)?)),
            ReplyStat::MsgDenied => Ok(ReplyBody::MsgDenied(deserialize(src)?)),
        }
    }
}

/// Information about program version mismatch
#[derive(Clone, Debug)]
pub struct MismatchInfo {
    /// Lowest version supported
    pub low: u32,
    /// Highest version supported
    pub high: u32,
}
DeserializeStruct!(MismatchInfo, low, high);
SerializeStruct!(MismatchInfo, low, high);

/// Reply to an RPC call that was accepted by the server.
///
/// Even though the call was accepted, there could still be an error in processing it.
/// The structure contains:
/// - An authentication verifier generated by the server to validate itself to the client
/// - A union containing the actual reply data, discriminated by `accept_stat` enum
///
/// The `reply_data` union has the following arms:
/// - `Success`: Contains protocol-specific success response
/// - `ProgUnavail`: Program not available (void)
/// - `ProgMismatch`: Program version mismatch, includes supported version range
/// - `ProcUnavail`: Procedure not available (void)
/// - `GarbageArgs`: Arguments could not be decoded (void)
#[derive(Clone, Debug, Default)]
pub struct AcceptedReply {
    /// Authentication verifier from server
    pub verf: OpaqueAuth,
    /// Reply data union discriminated by `accept_stat`
    pub reply_data: AcceptBody,
}
DeserializeStruct!(AcceptedReply, verf, reply_data);
SerializeStruct!(AcceptedReply, verf, reply_data);

/// Response data for an accepted RPC call, discriminated by `accept_stat`.
///
/// This enum represents the possible outcomes of an accepted RPC call:
/// - `Success`: Call completed successfully, response data is protocol-specific
/// - `ProgUnavail`: The requested program is not available on this server
/// - `ProgMismatch`: Program version mismatch, includes supported version range
/// - `ProcUnavail`: The requested procedure is not available in this program
/// - `GarbageArgs`: The server could not decode the call arguments
#[derive(Clone, Debug, Default)]
#[repr(u32)]
pub enum AcceptBody {
    /// Call completed successfully
    #[default]
    Success,
    /// Program is not available on this server
    ProgUnavail,
    /// Program version mismatch, includes supported version range
    ProgMismatch(MismatchInfo),
    /// Requested procedure is not available
    ProcUnavail,
    /// Server could not decode the call arguments
    GarbageArgs,
}

impl Serialize for AcceptBody {
    fn serialize<R: Write>(&self, dest: &mut R) -> std::io::Result<()> {
        match self {
            AcceptBody::Success => {
                AcceptStat::Success.serialize(dest)?;
            }
            AcceptBody::ProgUnavail => {
                AcceptStat::ProgUnavail.serialize(dest)?;
            }
            AcceptBody::ProgMismatch(v) => {
                AcceptStat::ProgMismatch.serialize(dest)?;
                v.serialize(dest)?;
            }
            AcceptBody::ProcUnavail => {
                AcceptStat::ProcUnavail.serialize(dest)?;
            }
            AcceptBody::GarbageArgs => {
                AcceptStat::GarbageAargs.serialize(dest)?;
            }
        }

        Ok(())
    }
}
impl Deserialize for AcceptBody {
    fn deserialize<R: Read>(src: &mut R) -> std::io::Result<Self> {
        Ok(match deserialize::<u32>(src)? {
            0 => AcceptBody::Success,
            1 => AcceptBody::ProgUnavail,
            2 => AcceptBody::ProgMismatch(deserialize(src)?),
            3 => AcceptBody::ProcUnavail,
            4 => AcceptBody::GarbageArgs,
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
/// 1. RPC Version Mismatch (`RPCMismatch`):
///    - Server is not running a compatible version of the RPC protocol
///    - Server returns the lowest and highest supported RPC versions
///
/// 2. Authentication Error (`AuthError`):
///    - Server refuses to authenticate the caller
///    - Returns specific auth failure status code
///
/// The discriminant for this enum is [`RejectStat`] which indicates the
/// rejection reason.
#[derive(Clone, Debug)]
pub enum RejectedReply {
    /// RPC version mismatch - includes supported version range
    RPCMismatch(MismatchInfo),
    /// Authentication failed - includes specific error code
    AuthError(AuthStat),
}

impl Default for RejectedReply {
    fn default() -> RejectedReply {
        RejectedReply::AuthError(AuthStat::default())
    }
}

impl Serialize for RejectedReply {
    fn serialize<R: Write>(&self, dest: &mut R) -> std::io::Result<()> {
        match self {
            RejectedReply::RPCMismatch(v) => {
                RejectStat::RPCMismatch.serialize(dest)?;
                v.serialize(dest)?;
            }
            RejectedReply::AuthError(v) => {
                RejectStat::AuthError.serialize(dest)?;
                (*v as u32).serialize(dest)?;
            }
        }

        Ok(())
    }
}
impl Deserialize for RejectedReply {
    fn deserialize<R: Read>(src: &mut R) -> std::io::Result<Self> {
        Ok(match deserialize::<u32>(src)? {
            0 => RejectedReply::RPCMismatch(deserialize(src)?),
            1 => RejectedReply::AuthError(deserialize(src)?),
            stat => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Invalid reject stat in rejected_reply: {stat}"),
                ))
            }
        })
    }
}

/// Creates a reply message indicating that the requested procedure is not available
pub fn proc_unavail_reply_message(xid: u32) -> RPCMsg {
    let reply = ReplyBody::MsgAccepted(AcceptedReply {
        verf: OpaqueAuth::default(),
        reply_data: AcceptBody::ProcUnavail,
    });
    RPCMsg { xid, body: RPCBody::Reply(reply) }
}

/// Creates a reply message indicating that the requested program is not available
pub fn prog_unavail_reply_message(xid: u32) -> RPCMsg {
    let reply = ReplyBody::MsgAccepted(AcceptedReply {
        verf: OpaqueAuth::default(),
        reply_data: AcceptBody::ProgUnavail,
    });
    RPCMsg { xid, body: RPCBody::Reply(reply) }
}

/// Creates a reply message indicating a program version mismatch
pub fn prog_mismatch_reply_message(xid: u32, accepted_ver: u32) -> RPCMsg {
    let reply = ReplyBody::MsgAccepted(AcceptedReply {
        verf: OpaqueAuth::default(),
        reply_data: AcceptBody::ProgMismatch(MismatchInfo {
            low: accepted_ver,
            high: accepted_ver,
        }),
    });
    RPCMsg { xid, body: RPCBody::Reply(reply) }
}

/// Creates a reply message indicating a program version mismatch with supported version range
pub fn prog_version_range_mismatch_reply_message(
    xid: u32,
    low_version: u32,
    high_version: u32,
) -> RPCMsg {
    let reply = ReplyBody::MsgAccepted(AcceptedReply {
        verf: OpaqueAuth::default(),
        reply_data: AcceptBody::ProgMismatch(MismatchInfo { low: low_version, high: high_version }),
    });
    RPCMsg { xid, body: RPCBody::Reply(reply) }
}

/// Creates a reply message indicating that the arguments could not be decoded
pub fn garbage_args_reply_message(xid: u32) -> RPCMsg {
    let reply = ReplyBody::MsgAccepted(AcceptedReply {
        verf: OpaqueAuth::default(),
        reply_data: AcceptBody::GarbageArgs,
    });
    RPCMsg { xid, body: RPCBody::Reply(reply) }
}

/// Creates a reply message indicating an RPC version mismatch
pub fn rpc_vers_mismatch(xid: u32) -> RPCMsg {
    let mismatch_info = MismatchInfo { low: PROTOCOL_VERSION, high: PROTOCOL_VERSION };
    let reply = ReplyBody::MsgDenied(RejectedReply::RPCMismatch(mismatch_info));
    RPCMsg { xid, body: RPCBody::Reply(reply) }
}

/// Creates a successful reply message with no additional data
pub fn make_success_reply(xid: u32) -> RPCMsg {
    let reply = ReplyBody::MsgAccepted(AcceptedReply {
        verf: OpaqueAuth::default(),
        reply_data: AcceptBody::Success,
    });
    RPCMsg { xid, body: RPCBody::Reply(reply) }
}
