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

use std::io::{self, Cursor, Read};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use tokio::net::TcpStream;
use tokio::time::{self, Duration};

use crate::consts::mount::MOUNT_PROGRAM;
use crate::consts::nfsv3::NFS_PROGRAM;
use crate::consts::nlm::NLM_PROGRAM;
use crate::rpc::{AcceptStat, AuthFlavor, RpcBody};

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

/// Version of the NFS service registered with `rpcbind`.
const NFS_REG_VERSION: u32 = 3;
/// Version of the MOUNT service registered with `rpcbind`.
const MOUNT_REG_VERSION: u32 = 3;
/// Version of the NLM service registered with `rpcbind`.
const NLM_REG_VERSION: u32 = 4;

/// RPC `reply_stat` for an accepted, matched message.
const MSG_ACCEPTED: u32 = 0;

/// MSB of the TCP record marker flags the last (and only) fragment of a
/// record. Per RFC 5531 section 11 every record must set it; without it the
/// peer keeps waiting for further fragments. The low 31 bits hold the length.
const LAST_FRAG: u32 = 0x8000_0000;

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
/// All three programs (NFS, MOUNT and NLM) share a single listening port.
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
        Mapping::new(NFS_PROGRAM, NFS_REG_VERSION, port),
        Mapping::new(MOUNT_PROGRAM, MOUNT_REG_VERSION, port),
        Mapping::new(NLM_PROGRAM, NLM_REG_VERSION, port),
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
    msg.write_u32::<BigEndian>(xid)?;
    msg.write_u32::<BigEndian>(RpcBody::Call as u32)?;
    msg.write_u32::<BigEndian>(crate::rpc::RPC_VERSION)?;
    msg.write_u32::<BigEndian>(PMAP_PROGRAM)?;
    msg.write_u32::<BigEndian>(PMAP_VERSION)?;
    msg.write_u32::<BigEndian>(proc)?;

    // Null credential and verifier (AUTH_NONE): flavor 0, empty opaque body.
    msg.write_u32::<BigEndian>(AuthFlavor::None as u32)?;
    msg.write_u32::<BigEndian>(0)?;
    msg.write_u32::<BigEndian>(AuthFlavor::None as u32)?;
    msg.write_u32::<BigEndian>(0)?;

    // PMAP mapping body: { prog, vers, prot, port }.
    msg.write_u32::<BigEndian>(mapping.prog)?;
    msg.write_u32::<BigEndian>(mapping.vers)?;
    msg.write_u32::<BigEndian>(mapping.prot)?;
    msg.write_u32::<BigEndian>(mapping.port as u32)?;

    let msg = msg.into_inner();

    // TCP record marker: 4-byte BE length prefix (with LAST_FRAG set,
    // since a request is always a single fragment) + message body.
    let mut record = Vec::with_capacity(msg.len() + 4);
    record.write_u32::<BigEndian>(LAST_FRAG | usize_to_u32(msg.len())?)?;
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

    let stream = match time::timeout(RPC_TIMEOUT, TcpStream::connect(addr)).await {
        Ok(Ok(stream)) => stream,
        Ok(Err(err)) => {
            return Err(io::Error::new(
                err.kind(),
                format!("cannot reach rpcbind at {addr}: {err}"),
            ))
        }
        Err(_) => {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!("connect to rpcbind at {addr} timed out"),
            ))
        }
    };

    if let Err(err) = stream.writable().await {
        return Err(io::Error::new(
            err.kind(),
            format!("prepare write to rpcbind at {addr} failed: {err}"),
        ));
    }
    if let Err(err) = stream.try_write(&record) {
        return Err(io::Error::new(
            err.kind(),
            format!("write to rpcbind at {addr} failed: {err}"),
        ));
    }

    let reply = time::timeout(RPC_TIMEOUT, async {
        let body = read_record(&stream).await?;
        verify_reply(&body, xid)
    })
    .await;

    match reply {
        Ok(result) => result,
        Err(_) => Err(io::Error::new(
            io::ErrorKind::TimedOut,
            format!("reply from rpcbind at {addr} timed out"),
        )),
    }
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
/// Returns `InvalidData` if the reply is fragmented, `UnexpectedEof` if the
/// peer closes the connection early, otherwise the underlying I/O error.
async fn read_record(stream: &TcpStream) -> io::Result<Vec<u8>> {
    let mut marker_buf = [0u8; 4];
    read_exact(stream, &mut marker_buf).await?;
    let marker = u32::from_be_bytes(marker_buf);
    if marker & LAST_FRAG == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "fragmented rpcbind reply not supported",
        ));
    }
    let record_len = (marker & !LAST_FRAG) as usize;

    let mut body = vec![0u8; record_len];
    read_exact(stream, &mut body).await?;
    Ok(body)
}

/// Validates an `rpcbind` reply body against the sent transaction id.
///
/// Checks that the reply carries the expected xid, reports `accept_stat ==
/// SUCCESS` and a boolean result of `TRUE`.
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

    if reader.read_u32::<BigEndian>()? != xid {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "rpcbind reply xid mismatch"));
    }
    if reader.read_u32::<BigEndian>()? != RpcBody::Reply as u32 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "rpcbind did not return a reply"));
    }
    if reader.read_u32::<BigEndian>()? != MSG_ACCEPTED {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "rpcbind reply denied"));
    }

    // Reply verifier: AUTH_NONE flavor with an acceptably-sized optional body.
    let verf_flavor = reader.read_u32::<BigEndian>()?;
    let verf_len = reader.read_u32::<BigEndian>()? as usize;
    if verf_flavor != AuthFlavor::None as u32 || verf_len > 400 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "unexpected rpcbind verifier"));
    }
    let mut skip = [0u8; 4];
    for _ in 0..(verf_len / 4) {
        reader.read_exact(&mut skip)?;
    }

    if reader.read_u32::<BigEndian>()? != AcceptStat::Success as u32 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "rpcbind rejected the call"));
    }
    if reader.read_u32::<BigEndian>()? != 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "rpcbind reported FALSE for the request",
        ));
    }

    Ok(())
}

/// Reads exactly `buf.len()` bytes from `stream`, tolerating `WouldBlock`.
///
/// # Parameters
///
/// * `stream` - Connected TCP stream to read from.
/// * `buf` - Buffer to fill.
///
/// # Returns
///
/// `Ok(())` once the buffer is full.
///
/// # Errors
///
/// Returns `UnexpectedEof` if the peer closes the connection early, otherwise
/// the underlying I/O error.
async fn read_exact(stream: &TcpStream, buf: &mut [u8]) -> io::Result<()> {
    let mut read = 0;
    while read < buf.len() {
        stream.readable().await?;
        match stream.try_read(&mut buf[read..]) {
            Ok(0) => return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "connection closed")),
            Ok(n) => read += n,
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => continue,
            Err(e) => return Err(e),
        }
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
/// registration. A failure here is not fatal to the server itself: clients can
/// still mount with explicit `port=`/`mountport=` options.
pub async fn register(port: u16) -> io::Result<()> {
    for mapping in server_mappings(port) {
        send_rpc_call(mapping, PMAP_PROC_SET).await?;
    }
    Ok(())
}

/// Withdraws previously registered NFS, MOUNT and NLM mappings from `rpcbind`.
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
/// Returns an `io::Error` if `rpcbind` is unreachable, times out or rejects a
/// withdrawal. The caller should treat the failure as best-effort.
pub async fn unregister(port: u16) -> io::Result<()> {
    for mapping in server_mappings(port) {
        send_rpc_call(mapping, PMAP_PROC_UNSET).await?;
    }
    Ok(())
}
