//! Minimal ONC RPC client for NFSv3 and MOUNT over TCP.
//!
//! The library keeps its client-side serializers private, so the few request builders
//! needed by `nfs-bench` and by the integration tests are re-implemented here on top
//! of a tiny XDR layer. Calls carry `AUTH_NONE` credentials.
//!
//! Two levels are provided:
//!
//! - record framing and reply decoding ([`call_record`], [`read_record`],
//!   [`parse_reply`]) for callers that pipeline many requests on one socket;
//! - [`Connection`], a sequential request/reply client used for setup and in tests.

pub mod mount;
pub mod nfs;
pub mod xdr;

use std::fmt;
use std::io;
use std::net::SocketAddr;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use nfs_mamont::consts::rpc::RPC_VERSION;

use self::xdr::{put_u32, Reader};

/// Bit of the record marker flagging the last fragment of a record.
pub const LAST_FRAGMENT: u32 = 0x8000_0000;
/// Size of the record marker.
pub const RM_HEADER_SIZE: usize = 4;
/// Size of an RPC call header with `AUTH_NONE` credential and verifier: `xid`,
/// `msg_type`, `rpcvers`, `prog`, `vers`, `proc`, two `opaque_auth` of two words each.
pub const CALL_HEADER_SIZE: usize = 10 * 4;
/// Offset of the first argument byte inside a record built by [`call_record`].
pub const ARGS_OFFSET: usize = RM_HEADER_SIZE + CALL_HEADER_SIZE;
/// Largest reply record accepted, a guard against garbage on the wire.
pub const MAX_RECORD_SIZE: usize = 256 << 20;

const MSG_CALL: u32 = 0;
const MSG_REPLY: u32 = 1;
const MSG_ACCEPTED: u32 = 0;
const MSG_DENIED: u32 = 1;
const AUTH_NONE: u32 = 0;

/// Error of a client call.
#[derive(Debug)]
pub enum Error {
    /// Socket error.
    Io(io::Error),
    /// The reply does not have the expected shape.
    Malformed(&'static str),
    /// The RPC layer did not execute the procedure.
    Rpc(RpcError),
    /// The procedure ran and returned a non-zero status (`nfsstat3`, `mountstat3`).
    Status(u32),
    /// The reply answers another request.
    XidMismatch {
        /// Transaction id of the request.
        expected: u32,
        /// Transaction id found in the reply.
        got: u32,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(err) => write!(f, "i/o error: {err}"),
            Error::Malformed(what) => write!(f, "malformed reply: {what}"),
            Error::Rpc(err) => write!(f, "rpc error: {err:?}"),
            Error::Status(code) => write!(f, "status {code} ({})", status_name(*code)),
            Error::XidMismatch { expected, got } => {
                write!(f, "reply xid {got} does not match request xid {expected}")
            }
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Error::Io(err)
    }
}

/// Reason the RPC layer gave for not executing a procedure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RpcError {
    /// `MSG_DENIED` with the `rejected_reply` discriminant and its first word.
    Denied {
        /// `RPC_MISMATCH` (0) or `AUTH_ERROR` (1).
        reason: u32,
        /// Low RPC version or `auth_stat`, depending on `reason`.
        detail: u32,
    },
    ProgUnavail,
    ProgMismatch {
        low: u32,
        high: u32,
    },
    ProcUnavail,
    GarbageArgs,
    SystemErr,
    Unknown(u32),
}

/// Returns the symbolic name of common `nfsstat3`/`mountstat3` codes.
pub fn status_name(code: u32) -> &'static str {
    match code {
        0 => "OK",
        1 => "PERM",
        2 => "NOENT",
        5 => "IO",
        13 => "ACCES",
        17 => "EXIST",
        20 => "NOTDIR",
        21 => "ISDIR",
        22 => "INVAL",
        28 => "NOSPC",
        30 => "ROFS",
        63 => "NAMETOOLONG",
        66 => "NOTEMPTY",
        70 => "STALE",
        10001 => "BADHANDLE",
        10002 => "NOT_SYNC",
        10003 => "BAD_COOKIE",
        10004 => "NOTSUPP",
        10005 => "TOOSMALL",
        10006 => "SERVERFAULT",
        10007 => "BADTYPE",
        10008 => "JUKEBOX",
        _ => "?",
    }
}

/// Builds a complete record (record marker, call header, arguments) for an
/// `AUTH_NONE` call. The result is a single fragment, as the server requires.
pub fn call_record(program: u32, version: u32, procedure: u32, xid: u32, args: &[u8]) -> Vec<u8> {
    let body_len = CALL_HEADER_SIZE + args.len();
    let mut record = Vec::with_capacity(RM_HEADER_SIZE + body_len);
    put_u32(&mut record, LAST_FRAGMENT | body_len as u32);
    put_u32(&mut record, xid);
    put_u32(&mut record, MSG_CALL);
    put_u32(&mut record, RPC_VERSION);
    put_u32(&mut record, program);
    put_u32(&mut record, version);
    put_u32(&mut record, procedure);
    // Credential and verifier: AUTH_NONE with an empty body.
    put_u32(&mut record, AUTH_NONE);
    put_u32(&mut record, 0);
    put_u32(&mut record, AUTH_NONE);
    put_u32(&mut record, 0);
    record.extend_from_slice(args);
    record
}

/// Rewrites the transaction id of a record built by [`call_record`].
pub fn set_xid(record: &mut [u8], xid: u32) {
    record[RM_HEADER_SIZE..RM_HEADER_SIZE + 4].copy_from_slice(&xid.to_be_bytes());
}

/// Reads one record (all its fragments) into `buf`, replacing its content.
pub async fn read_record<R: AsyncRead + Unpin>(
    reader: &mut R,
    buf: &mut Vec<u8>,
) -> io::Result<()> {
    buf.clear();
    loop {
        let mut marker = [0u8; RM_HEADER_SIZE];
        reader.read_exact(&mut marker).await?;
        let marker = u32::from_be_bytes(marker);
        let len = (marker & !LAST_FRAGMENT) as usize;
        if buf.len() + len > MAX_RECORD_SIZE {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "reply record too large"));
        }

        let start = buf.len();
        buf.resize(start + len, 0);
        reader.read_exact(&mut buf[start..]).await?;
        if marker & LAST_FRAGMENT != 0 {
            return Ok(());
        }
    }
}

/// Decoded RPC reply header.
#[derive(Debug)]
pub struct Reply {
    /// Transaction id the reply answers.
    pub xid: u32,
    /// Offset of the procedure results inside the record, or why there are none.
    pub status: Result<usize, RpcError>,
}

/// Decodes the RPC header of a reply record.
pub fn parse_reply(record: &[u8]) -> Result<Reply, Error> {
    let mut reader = Reader::new(record);
    let xid = reader.u32()?;
    if reader.u32()? != MSG_REPLY {
        return Err(Error::Malformed("record is not an RPC reply"));
    }

    let status = match reader.u32()? {
        MSG_ACCEPTED => {
            let _verifier_flavor = reader.u32()?;
            reader.opaque()?;
            match reader.u32()? {
                0 => Ok(reader.position()),
                1 => Err(RpcError::ProgUnavail),
                2 => Err(RpcError::ProgMismatch { low: reader.u32()?, high: reader.u32()? }),
                3 => Err(RpcError::ProcUnavail),
                4 => Err(RpcError::GarbageArgs),
                5 => Err(RpcError::SystemErr),
                other => Err(RpcError::Unknown(other)),
            }
        }
        MSG_DENIED => {
            let reason = reader.u32()?;
            let detail = reader.u32().unwrap_or(0);
            Err(RpcError::Denied { reason, detail })
        }
        _ => return Err(Error::Malformed("unknown reply_stat")),
    };

    Ok(Reply { xid, status })
}

/// Sequential request/reply client over one TCP connection.
pub struct Connection {
    stream: TcpStream,
    next_xid: u32,
    buf: Vec<u8>,
}

impl Connection {
    /// Connects to `addr` with Nagle's algorithm disabled.
    pub async fn connect(addr: SocketAddr) -> io::Result<Self> {
        let stream = TcpStream::connect(addr).await?;
        let _ = stream.set_nodelay(true);

        Ok(Self { stream, next_xid: 1, buf: Vec::with_capacity(4096) })
    }

    /// Sends one call and waits for its reply.
    ///
    /// # Returns
    ///
    /// The procedure results (the reply body after the RPC header), valid until the
    /// next call.
    pub async fn call(
        &mut self,
        program: u32,
        version: u32,
        procedure: u32,
        args: &[u8],
    ) -> Result<&[u8], Error> {
        let xid = self.next_xid;
        self.next_xid = self.next_xid.wrapping_add(1);

        let record = call_record(program, version, procedure, xid, args);
        self.stream.write_all(&record).await?;
        read_record(&mut self.stream, &mut self.buf).await?;

        let reply = parse_reply(&self.buf)?;
        if reply.xid != xid {
            return Err(Error::XidMismatch { expected: xid, got: reply.xid });
        }
        let body = reply.status.map_err(Error::Rpc)?;

        Ok(&self.buf[body..])
    }

    /// Returns the underlying socket.
    pub fn into_stream(self) -> TcpStream {
        self.stream
    }
}

#[cfg(test)]
mod tests {
    use super::{call_record, parse_reply, set_xid, RpcError, ARGS_OFFSET, LAST_FRAGMENT};
    use crate::client::xdr::put_u32;

    #[test]
    fn call_record_is_a_single_fragment_with_args_at_the_documented_offset() {
        let record = call_record(100003, 3, 1, 7, &[0xAA, 0xBB, 0xCC, 0xDD]);

        let marker = u32::from_be_bytes([record[0], record[1], record[2], record[3]]);
        assert_eq!(marker & LAST_FRAGMENT, LAST_FRAGMENT);
        assert_eq!((marker & !LAST_FRAGMENT) as usize, record.len() - 4);
        assert_eq!(&record[ARGS_OFFSET..], &[0xAA, 0xBB, 0xCC, 0xDD]);
        assert_eq!(&record[4..8], &7u32.to_be_bytes());
    }

    #[test]
    fn set_xid_rewrites_the_transaction_id() {
        let mut record = call_record(100003, 3, 0, 1, &[]);
        set_xid(&mut record, 0xDEAD_BEEF);

        assert_eq!(parse_reply_xid(&record), 0xDEAD_BEEF);
    }

    fn parse_reply_xid(record: &[u8]) -> u32 {
        u32::from_be_bytes([record[4], record[5], record[6], record[7]])
    }

    #[test]
    fn accepted_reply_points_at_the_body() {
        let mut record = Vec::new();
        for word in [42, 1, 0, 0, 0, 0] {
            put_u32(&mut record, word);
        }
        put_u32(&mut record, 0x1234);

        let reply = parse_reply(&record).unwrap();
        assert_eq!(reply.xid, 42);
        assert_eq!(reply.status, Ok(24));
        assert_eq!(&record[24..], &0x1234u32.to_be_bytes());
    }

    #[test]
    fn garbage_args_and_denied_replies_are_reported() {
        let mut record = Vec::new();
        for word in [1, 1, 0, 0, 0, 4] {
            put_u32(&mut record, word);
        }
        assert_eq!(parse_reply(&record).unwrap().status, Err(RpcError::GarbageArgs));

        let mut denied = Vec::new();
        for word in [2, 1, 1, 1, 7] {
            put_u32(&mut denied, word);
        }
        assert_eq!(
            parse_reply(&denied).unwrap().status,
            Err(RpcError::Denied { reason: 1, detail: 7 })
        );
    }
}
