//! Client for the `rpcbind` (portmapper, program 100000) service.
//!
//! [NFS mount](https://man7.org/linux/man-pages/man5/nfs.5.html) on a client
//! implicitly queries the server's `rpcbind` on port 111 to discover the ports
//! of the MOUNT program (100005) and the NFS program (100003). When those ports
//! are not pinned via `mount` options like `port=`/`mountport=`, the server must
//! register its services with `rpcbind` so the client can resolve them.
//!
//! This module acts as an `rpcbind` *client*: it sends `PMAPPROC_SET` /
//! `PMAPPROC_UNSET` calls over TCP to a local `rpcbind` to publish/withdraw this
//! server's program-to-port mapping. Registration is best-effort: if `rpcbind`
//! is unavailable the server still works, but clients must then mount with
//! explicit `port=`/`mountport=` options.

#[cfg(test)]
mod tests;

use std::io::{self, Cursor};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::{self, Duration};
use tracing::warn;

use crate::consts::mount::{MOUNT_PROGRAM, MOUNT_VERSION};
use crate::consts::nfsv3::{NFS_PROGRAM, NFS_VERSION};
use crate::consts::nlm::{NLM_PROGRAM, NLM_VERSION};
use crate::parser::primitive;
use crate::parser::rpc as parser_rpc;
use crate::rpc::{AcceptStat, AuthFlavor, OpaqueAuth, ReplyBody, RpcBody, RPC_VERSION};
use crate::serializer;

/// Program number of the portmapper/`rpcbind` service itself.
const PMAP_PROGRAM: u32 = 100000;
/// Version of the portmapper protocol spoken here.
const PMAP_VERSION: u32 = 2;
/// `PMAPPROC_SET`: register/update a program mapping.
const PMAP_PROC_SET: u32 = 1;
/// `PMAPPROC_UNSET`: remove a program mapping.
const PMAP_PROC_UNSET: u32 = 2;
/// IP protocol number for TCP as used in a `mapping`.
const IPPROTO_TCP: u32 = 6;

/// MSB of the TCP record marker flags the last (and only) fragment of a
/// record. Per RFC 5531 section 11 every record must set it; without it the
/// peer keeps waiting for further fragments. The low 31 bits hold the length.
const LAST_FRAG: u32 = 0x8000_0000;

/// Upper bound on the length of a reply record accepted from `rpcbind`.
///
/// A portmapper reply is a few dozen bytes, but the 31 length bits of a record
/// marker allow values up to 2 GiB. Without a cap a hostile or broken peer
/// could make the client allocate that much memory for a single reply.
const MAX_RECORD_LEN: usize = 4096;

/// Well-known address of the local `rpcbind` service (ONC port 111).
const RPCBIND_ADDR: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 111);

/// Time budget for a single TCP round-trip to the `rpcbind` endpoint.
const RPC_TIMEOUT: Duration = Duration::from_secs(5);

/// A single program-to-port registration record (the XDR `mapping` type).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Mapping {
    /// RPC program number to register (e.g. 100003 for NFS).
    prog: u32,
    /// Program version to register (e.g. 3 for NFSv3).
    vers: u32,
    /// IP protocol: `IPPROTO_TCP` (6) for connection-oriented transports.
    prot: u32,
    /// Port on which the program listens.
    port: u16,
}

impl Mapping {
    /// Creates a TCP mapping for the given program and port.
    ///
    /// The protocol is fixed to TCP, the only transport this server uses.
    ///
    /// # Parameters
    ///
    /// * `prog` - RPC program number to register (e.g. 100003 for NFS).
    /// * `vers` - Program version to register (e.g. 3 for NFSv3).
    /// * `port` - Port on which the program listens.
    ///
    /// # Returns
    ///
    /// A mapping with the protocol field set to `IPPROTO_TCP`.
    const fn new(prog: u32, vers: u32, port: u16) -> Self {
        Self { prog, vers, prot: IPPROTO_TCP, port }
    }
}

/// Returns the three service mappings served by the NFS server.
///
/// All three programs (NFS, MOUNT and NLM) share a single listening port. The
/// advertised versions are the [`crate::consts`] ones the server actually
/// implements, so a version bump cannot leave `rpcbind` advertising a version
/// the server does not answer.
///
/// # Parameters
///
/// * `port` - Port on which the server listens.
///
/// # Returns
///
/// The mappings for programs 100003/3, 100005/3 and 100021/4 over TCP.
const fn server_mappings(port: u16) -> [Mapping; 3] {
    [
        Mapping::new(NFS_PROGRAM, NFS_VERSION, port),
        Mapping::new(MOUNT_PROGRAM, MOUNT_VERSION, port),
        Mapping::new(NLM_PROGRAM, NLM_VERSION, port),
    ]
}

/// Converts a record length to a `u32` for the TCP record marker.
///
/// # Parameters
///
/// * `n` - Length of the XDR-encoded message in bytes.
///
/// # Returns
///
/// The length as a `u32` if it fits.
///
/// # Errors
///
/// Returns `InvalidInput` if `n` exceeds `u32::MAX`.
fn usize_to_u32(n: usize) -> io::Result<u32> {
    u32::try_from(n)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "RPC record too large"))
}

/// Builds the null (`AUTH_NONE`) credential/verifier used by every call.
///
/// # Returns
///
/// An [`OpaqueAuth`] with the `AUTH_NONE` flavor and an empty body.
fn null_auth() -> OpaqueAuth {
    OpaqueAuth { flavor: AuthFlavor::None, body: Vec::new() }
}

/// Serializes an `rpcbind` RPC call message plus its TCP record marker.
///
/// The record framing uses a 4-byte big-endian length prefix followed by the
/// XDR-encoded message, exactly as `rpcbind` expects over the TCP transport.
///
/// # Parameters
///
/// * `mapping` - Program-to-port mapping to send in the call body.
/// * `xid` - Transaction id echoed by the peer in the reply.
/// * `proc` - PMAPPROC_* procedure number to invoke.
///
/// # Returns
///
/// The fully framed record: a 4-byte marker (with the last-fragment bit set)
/// followed by the XDR call message.
///
/// # Errors
///
/// Returns `InvalidInput` if the message is too large to fit a record marker.
fn encode_call(mapping: Mapping, xid: u32, proc: u32) -> io::Result<Vec<u8>> {
    let mut msg = Cursor::new(Vec::with_capacity(64));

    // RPC header.
    serializer::u32(&mut msg, xid)?;
    serializer::u32(&mut msg, RpcBody::Call as u32)?;
    serializer::u32(&mut msg, RPC_VERSION)?;
    serializer::u32(&mut msg, PMAP_PROGRAM)?;
    serializer::u32(&mut msg, PMAP_VERSION)?;
    serializer::u32(&mut msg, proc)?;

    // Null credential and verifier (AUTH_NONE): flavor 0, empty opaque body.
    serializer::server::rpc::auth(&mut msg, null_auth())?;
    serializer::server::rpc::auth(&mut msg, null_auth())?;

    // PMAP mapping body: { prog, vers, prot, port }.
    serializer::u32(&mut msg, mapping.prog)?;
    serializer::u32(&mut msg, mapping.vers)?;
    serializer::u32(&mut msg, mapping.prot)?;
    serializer::u32(&mut msg, mapping.port as u32)?;

    let msg = msg.into_inner();

    // TCP record marker: 4-byte BE length prefix (with LAST_FRAG set,
    // since a request is always a single fragment) + message body.
    let mut record = Vec::with_capacity(msg.len() + 4);
    serializer::u32(&mut record, LAST_FRAG | usize_to_u32(msg.len())?)?;
    record.extend_from_slice(&msg);
    Ok(record)
}

/// Derives a per-call transaction id from the call parameters.
///
/// # Parameters
///
/// * `mapping` - Program-to-port mapping of the call.
/// * `proc` - PMAPPROC_* procedure number to invoke.
///
/// # Returns
///
/// A deterministic xid for the call, distinct from the ids of other calls.
fn xid_for(mapping: Mapping, proc: u32) -> u32 {
    mapping.prog.rotate_left(16) ^ mapping.vers.rotate_left(8) ^ mapping.port as u32 ^ proc
}

/// Sends a single `rpcbind` RPC call to the local `rpcbind` and checks the reply.
///
/// # Parameters
///
/// * `mapping` - Program-to-port mapping to register or withdraw.
/// * `proc` - PMAPPROC_SET or PMAPPROC_UNSET to invoke.
///
/// # Returns
///
/// `Ok(())` if `rpcbind` accepted the call and reported a successful result.
///
/// # Errors
///
/// Returns an `io::Error` if `rpcbind` is unreachable, times out, or replies
/// with a denial or a `FALSE` result.
async fn send_rpc_call(mapping: Mapping, proc: u32) -> io::Result<()> {
    send_rpc_call_to(RPCBIND_ADDR, mapping, proc).await
}

/// Like [`send_rpc_call`], but targets an explicit `addr` (useful for tests).
///
/// The whole exchange - connect, write, read and verify - is bounded by a
/// single [`RPC_TIMEOUT`], so no stage can block the caller indefinitely.
///
/// # Parameters
///
/// * `addr` - Address of the `rpcbind` endpoint to talk to.
/// * `mapping` - Program-to-port mapping to register or withdraw.
/// * `proc` - PMAPPROC_SET or PMAPPROC_UNSET to invoke.
///
/// # Returns
///
/// `Ok(())` if the endpoint accepted the call and reported a successful result.
///
/// # Errors
///
/// Returns an `io::Error` if the endpoint is unreachable, times out, or replies
/// with a denial or a `FALSE` result.
async fn send_rpc_call_to(addr: SocketAddr, mapping: Mapping, proc: u32) -> io::Result<()> {
    let xid = xid_for(mapping, proc);
    let record = encode_call(mapping, xid, proc)?;

    match time::timeout(RPC_TIMEOUT, exchange(addr, &record, xid)).await {
        Ok(result) => result,
        Err(_) => Err(io::Error::new(
            io::ErrorKind::TimedOut,
            format!("call to rpcbind at {addr} timed out"),
        )),
    }
}

/// Performs one request/reply exchange with an `rpcbind` endpoint.
///
/// # Parameters
///
/// * `addr` - Address of the `rpcbind` endpoint to talk to.
/// * `record` - Fully framed call record to send.
/// * `xid` - Transaction id expected back in the reply.
///
/// # Returns
///
/// `Ok(())` if the endpoint replied with a matching, successful reply.
///
/// # Errors
///
/// Returns an `io::Error` describing the stage that failed: connecting,
/// writing, reading, or verifying the reply.
async fn exchange(addr: SocketAddr, record: &[u8], xid: u32) -> io::Result<()> {
    let mut stream = TcpStream::connect(addr).await.map_err(|err| {
        io::Error::new(err.kind(), format!("cannot reach rpcbind at {addr}: {err}"))
    })?;

    stream.write_all(record).await.map_err(|err| {
        io::Error::new(err.kind(), format!("write to rpcbind at {addr} failed: {err}"))
    })?;

    let body = read_record(&mut stream).await.map_err(|err| {
        io::Error::new(err.kind(), format!("read from rpcbind at {addr} failed: {err}"))
    })?;

    verify_reply(&body, xid)
}

/// Reads one single-fragment TCP-recorded RPC reply body.
///
/// Per RFC 5531 section 11 every TCP record sets the last-fragment bit (MSB)
/// on its marker, so a reply that does not set it is unsupported and rejected.
///
/// # Parameters
///
/// * `stream` - Connected TCP stream to read from.
///
/// # Returns
///
/// The XDR body of the reply, without the 4-byte record marker.
///
/// # Errors
///
/// Returns `InvalidData` if the reply is fragmented or longer than
/// [`MAX_RECORD_LEN`], `UnexpectedEof` if the peer closes the connection early,
/// otherwise the underlying I/O error.
async fn read_record(stream: &mut TcpStream) -> io::Result<Vec<u8>> {
    let mut marker_buf = [0u8; 4];
    stream.read_exact(&mut marker_buf).await?;
    let marker = u32::from_be_bytes(marker_buf);
    if marker & LAST_FRAG == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "fragmented rpcbind reply not supported",
        ));
    }

    // The length is peer-controlled, so refuse to size an allocation by it
    // beyond what a portmapper reply can plausibly need.
    let record_len = (marker & !LAST_FRAG) as usize;
    if record_len > MAX_RECORD_LEN {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("rpcbind reply of {record_len} bytes exceeds the {MAX_RECORD_LEN} byte limit"),
        ));
    }

    let mut body = vec![0u8; record_len];
    stream.read_exact(&mut body).await?;
    Ok(body)
}

/// Converts a parse error from the shared XDR parser into an `io::Error`.
///
/// # Parameters
///
/// * `err` - Error reported while decoding the reply.
///
/// # Returns
///
/// The underlying I/O error, or `InvalidData` describing the malformed field.
fn parse_error(err: crate::rpc::Error) -> io::Error {
    match err {
        crate::rpc::Error::IO(err) => err,
        other => io::Error::new(
            io::ErrorKind::InvalidData,
            format!("malformed rpcbind reply: {other:?}"),
        ),
    }
}

/// Validates an `rpcbind` reply body against the sent transaction id.
///
/// Checks that the reply carries the expected xid, reports `accept_stat ==
/// SUCCESS` and a boolean result of `TRUE`. Decoding goes through the crate's
/// XDR [`primitive`] parser, so the verifier is length-checked against
/// [`crate::rpc::MAX_AUTH_SIZE`] and its XDR padding is consumed correctly.
///
/// # Parameters
///
/// * `body` - XDR reply body, without the record marker.
/// * `xid` - Transaction id of the sent call.
///
/// # Returns
///
/// `Ok(())` if the reply matches the call and reports success.
///
/// # Errors
///
/// Returns `InvalidData` on any mismatch: wrong xid, a missing `MSG_ACCEPTED`
/// marker, an unexpected verifier, a rejected call or a `FALSE` result.
fn verify_reply(body: &[u8], xid: u32) -> io::Result<()> {
    let mut reader = Cursor::new(body);

    if primitive::u32(&mut reader).map_err(parse_error)? != xid {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "rpcbind reply xid mismatch"));
    }
    if primitive::u32(&mut reader).map_err(parse_error)? != RpcBody::Reply as u32 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "rpcbind did not return a reply"));
    }
    if primitive::u32(&mut reader).map_err(parse_error)? != ReplyBody::MsgAccepted as u32 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "rpcbind reply denied"));
    }

    // Reply verifier: expected to be the null AUTH_NONE one we also send.
    let verifier = parser_rpc::auth(&mut reader).map_err(parse_error)?;
    if !matches!(verifier.flavor, AuthFlavor::None) {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "unexpected rpcbind verifier"));
    }

    if primitive::u32(&mut reader).map_err(parse_error)? != AcceptStat::Success as u32 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "rpcbind rejected the call"));
    }
    if !primitive::bool(&mut reader).map_err(parse_error)? {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "rpcbind reported FALSE for the request",
        ));
    }

    Ok(())
}

/// Registers the NFS, MOUNT and NLM services with the local `rpcbind`.
///
/// This publishes the fact that the given `port` serves programs 100003/3,
/// 100005/3 and 100021/4 over TCP, so a client can mount without supplying
/// explicit `port=`/`mountport=` options.
///
/// # Parameters
///
/// * `port` - Port on which the server listens.
///
/// # Returns
///
/// `Ok(())` if all three mappings were registered successfully.
///
/// # Errors
///
/// Returns an `io::Error` if `rpcbind` is unreachable, times out or rejects a
/// registration. In that case the mappings published so far are withdrawn
/// again, so `rpcbind` never keeps advertising a partially registered server.
/// A failure here is not fatal to the server itself: clients can still mount
/// with explicit `port=`/`mountport=` options.
pub async fn register(port: u16) -> io::Result<()> {
    let mappings = server_mappings(port);

    for (published, mapping) in mappings.iter().enumerate() {
        let Err(err) = send_rpc_call(*mapping, PMAP_PROC_SET).await else {
            continue;
        };

        // Roll back what was already published: leaving a half-registered
        // server behind would send clients to programs we never advertised.
        for mapping in &mappings[..published] {
            if let Err(err) = send_rpc_call(*mapping, PMAP_PROC_UNSET).await {
                warn!(
                    prog = mapping.prog,
                    error = %err,
                    "failed to roll back an rpcbind registration"
                );
            }
        }

        return Err(err);
    }

    Ok(())
}

/// Withdraws previously registered NFS, MOUNT and NLM mappings from `rpcbind`.
///
/// Every mapping is attempted even if an earlier one fails, so a single
/// failure cannot leave the remaining programs advertised.
///
/// # Parameters
///
/// * `port` - Port for which the mappings were registered.
///
/// # Returns
///
/// `Ok(())` if all three mappings were withdrawn successfully.
///
/// # Errors
///
/// Returns the first `io::Error` encountered if `rpcbind` is unreachable,
/// times out or rejects a withdrawal. The caller should treat the failure as
/// best-effort.
pub async fn unregister(port: u16) -> io::Result<()> {
    let mut result = Ok(());

    for mapping in server_mappings(port) {
        if let Err(err) = send_rpc_call(mapping, PMAP_PROC_UNSET).await {
            warn!(prog = mapping.prog, error = %err, "failed to withdraw an rpcbind mapping");
            if result.is_ok() {
                result = Err(err);
            }
        }
    }

    result
}
