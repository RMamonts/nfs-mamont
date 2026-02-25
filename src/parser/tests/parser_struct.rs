use crate::parser::parser_struct::RpcParser;
use crate::parser::tests::allocator::MockAllocator;
use crate::parser::tests::socket::MockSocket;
use crate::parser::Arguments;
use crate::parser::Error;

/// Constants for mock RPC/NFS test input construction.
const XID: u32 = 1;
const MSG_CALL: u32 = 0;
const RPC_VERSION: u32 = 2;
const NFS_PROGRAM: u32 = 100_003;
const NFS_VERSION: u32 = 3;
const PROC_FSSTAT: u32 = 18;
const PROC_WRITE: u32 = 7;
const AUTH_NONE: u32 = 0;

/// Mask for the fragment header flag in the message header.
const FRAGMENT_HEADER_MASK: u32 = 0x8000_0000;

/// Writes a 32-bit big-endian integer to a buffer.
#[inline]
pub fn push_u32(buf: &mut Vec<u8>, value: u32) {
    buf.extend_from_slice(&value.to_be_bytes());
}

/// Writes a 64-bit big-endian integer to a buffer.
#[inline]
pub fn push_u64(buf: &mut Vec<u8>, value: u64) {
    buf.extend_from_slice(&value.to_be_bytes());
}

/// Appends raw bytes to the buffer.
#[inline]
pub fn push_bytes(buf: &mut Vec<u8>, bytes: &[u8]) {
    buf.extend_from_slice(bytes);
}

/// Pads the buffer with zeros to ensure 4-byte alignment.
///
/// # Parameters
///
/// * `buf` - The buffer to pad.
/// * `content_len` - Length of the newly appended content that may require padding.
fn pad_to_alignment(buf: &mut Vec<u8>, content_len: usize) {
    let padding_needed = (4 - (content_len & 3)) & 3;
    buf.extend(std::iter::repeat_n(0, padding_needed));
}

/// Appends an XDR opaque field: 4-byte length prefix, data, then padding for 4-byte alignment.
///
/// # Parameters
///
/// * `buf` - Destination buffer
/// * `bytes` - Data to append as opaque
fn push_opaque(buf: &mut Vec<u8>, bytes: &[u8]) {
    // Write length prefix (XDR specifies u32 length)
    push_u32(buf, bytes.len().try_into().unwrap());

    // Write actual data
    if !bytes.is_empty() {
        push_bytes(buf, bytes);
    }

    // Pad to next multiple of 4 for XDR alignment
    pad_to_alignment(buf, bytes.len());
}

/// Constructs a complete NFS RPC call frame for the given procedure and arguments.
/// Returns the serialized byte buffer suitable for testing the parser.
fn nfs_call_frame(
    msg_type: u32,
    rpc_version: u32,
    auth_flavor: u32,
    procedure: u32,
    args_builder: impl FnOnce(&mut Vec<u8>),
) -> Vec<u8> {
    let mut payload = Vec::new();
    // RPC call header fields
    push_u32(&mut payload, XID);
    push_u32(&mut payload, msg_type);
    push_u32(&mut payload, rpc_version);
    push_u32(&mut payload, NFS_PROGRAM);
    push_u32(&mut payload, NFS_VERSION);
    push_u32(&mut payload, procedure);
    push_u32(&mut payload, auth_flavor);
    // Auth body length (0 for AUTH_NONE/SYS in tests)
    push_u32(&mut payload, 0);

    // Append procedure-specific arguments
    args_builder(&mut payload);

    let mut frame = Vec::with_capacity(payload.len() + 4);

    // Construct fragment header: set last fragment flag and include header size
    let payload_len: u32 = payload.len().try_into().unwrap();
    let fragment_header = FRAGMENT_HEADER_MASK | (payload_len + 4);

    push_u32(&mut frame, fragment_header);
    frame.extend_from_slice(&payload);
    frame
}

/// Serializes fsstat (NFS procedure 18) arguments: just a root file handle.
fn fsstat_args(root: [u8; 8]) -> Vec<u8> {
    let mut args = Vec::new();
    push_opaque(&mut args, &root);
    args
}

/// Serializes write (NFS procedure 7) arguments, including count, offset, stable, and data.
///
/// # Parameters
///
/// * `offset` - File offset to write to.
/// * `count` - Number of bytes to write.
/// * `stable` - Write stability.
/// * `data` - Data to write.
fn write_args(offset: u64, count: u32, stable: u32, data: &[u8]) -> Vec<u8> {
    let mut args = Vec::new();
    push_opaque(&mut args, &[1, 2, 3, 4, 5, 6, 7, 8]);
    push_u64(&mut args, offset);
    push_u32(&mut args, count);
    push_u32(&mut args, stable);
    push_u32(&mut args, data.len().try_into().unwrap());
    push_bytes(&mut args, data);
    pad_to_alignment(&mut args, data.len());
    args
}

/// Helper to assert parsed FSSTAT arguments are as expected.
fn assert_fsstat_result(result: &Arguments, expected_root: [u8; 8]) {
    match result {
        Arguments::FsStat(args) => assert_eq!(args.root.0, expected_root),
        _ => panic!("Wrong result type"),
    }
}

/// Test: Parses two correct NFS FSSTAT frames back-to-back.
#[tokio::test]
async fn parse_two_correct() {
    let first = nfs_call_frame(MSG_CALL, RPC_VERSION, AUTH_NONE, PROC_FSSTAT, |buf| {
        buf.extend_from_slice(&fsstat_args([1, 2, 3, 4, 5, 6, 7, 8]));
    });
    let second = nfs_call_frame(MSG_CALL, RPC_VERSION, AUTH_NONE, PROC_FSSTAT, |buf| {
        buf.extend_from_slice(&fsstat_args([1, 2, 3, 4, 5, 6, 7, 8]));
    });
    let mut buf = Vec::new();
    buf.extend_from_slice(&first);
    buf.extend_from_slice(&second);
    let socket = MockSocket::new(buf.as_slice());
    let alloc = MockAllocator::new(0);
    let mut parser = RpcParser::with_capacity(socket, alloc, 0x35);
    let _ = parser.parse_message().await;
    let result = parser.parse_message().await.unwrap();
    assert_fsstat_result(&result, [1, 2, 3, 4, 5, 6, 7, 8]);
}

/// Test: After a version mismatch error, parses the next valid FSSTAT frame.
#[tokio::test]
async fn parse_after_error() {
    let first = nfs_call_frame(MSG_CALL, 3, AUTH_NONE, PROC_FSSTAT, |buf| {
        buf.extend_from_slice(&fsstat_args([1, 2, 3, 4, 5, 6, 7, 8]));
    });
    let second = nfs_call_frame(MSG_CALL, RPC_VERSION, AUTH_NONE, PROC_FSSTAT, |buf| {
        buf.extend_from_slice(&fsstat_args([1, 2, 3, 4, 5, 6, 7, 8]));
    });
    let mut buf = Vec::new();
    buf.extend_from_slice(&first);
    buf.extend_from_slice(&second);
    let socket = MockSocket::new(buf.as_slice());
    let alloc = MockAllocator::new(0);
    let mut parser = RpcParser::with_capacity(socket, alloc, 0x50);
    let result = parser.parse_message().await;
    assert!(result.is_err());
    let result = parser.parse_message().await.unwrap();
    assert_fsstat_result(&result, [1, 2, 3, 4, 5, 6, 7, 8]);
}

/// Test: Parses two correct NFS WRITE frames with data.
#[tokio::test]
async fn parse_write() {
    let data = [
        0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x00, 0x00, 0x00, 0x08, 0x01, 0x02, 0x03,
        0x04, 0x01, 0x02,
    ];
    let first = nfs_call_frame(MSG_CALL, RPC_VERSION, AUTH_NONE, PROC_WRITE, |buf| {
        buf.extend_from_slice(&write_args(0x8000, 0xFF, 0, &data));
    });
    let second = nfs_call_frame(MSG_CALL, RPC_VERSION, AUTH_NONE, PROC_WRITE, |buf| {
        buf.extend_from_slice(&write_args(0x8000, 0xFF, 0, &data));
    });
    let mut buf = Vec::new();
    buf.extend_from_slice(&first);
    buf.extend_from_slice(&second);
    let socket = MockSocket::new(buf.as_slice());
    let alloc = MockAllocator::new(0x24);
    let mut parser = RpcParser::with_capacity(socket, alloc, 72);
    let result = parser.parse_message().await;
    assert!(result.is_ok());
    let result1 = parser.parse_message().await;
    assert!(result1.is_ok());
}

/// Test: Parser recovers from an error on first WRITE frame and parses the next valid WRITE frame.
#[tokio::test]
async fn parse_write_after_error() {
    let data = [
        0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x00, 0x00, 0x00, 0x08, 0x01, 0x02, 0x03,
        0x04, 0x01, 0x02,
    ];
    let first = nfs_call_frame(MSG_CALL, 5, AUTH_NONE, PROC_WRITE, |buf| {
        buf.extend_from_slice(&write_args(0x8000, 0xFF, 0, &data));
    });
    let second = nfs_call_frame(MSG_CALL, RPC_VERSION, AUTH_NONE, PROC_WRITE, |buf| {
        buf.extend_from_slice(&write_args(0x8000, 0xFF, 0, &data));
    });
    let mut buf = Vec::new();
    buf.extend_from_slice(&first);
    buf.extend_from_slice(&second);
    let socket = MockSocket::new(buf.as_slice());
    let alloc = MockAllocator::new(0x24);
    let mut parser = RpcParser::with_capacity(socket, alloc, 80);
    let result = parser.parse_message().await;
    assert!(result.is_err());
    let result = parser.parse_message().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn parse_error_when_consumed_exceeds_frame_size() {
    #[rustfmt::skip]
    let buf = vec![
        0x80, 0x00, 0x00, 0x04, // head with too small frame size
        0x00, 0x00, 0x00, 0x01, // xid
        0x00, 0x00, 0x00, 0x00, // request
        0x00, 0x00, 0x00, 0x05, // invalid rpc version (must be 2)
    ];
    let socket = MockSocket::new(buf.as_slice());
    let alloc = MockAllocator::new(0);
    let mut parser = RpcParser::with_capacity(socket, alloc, 0x20);

    let result = parser.parse_message().await;
    let error = result.err().unwrap();
    assert!(matches!(error, Error::IO(io_err) if io_err.kind() == std::io::ErrorKind::InvalidData));
}

#[tokio::test]
async fn parse_rejects_any_non_call_message_type() {
    #[rustfmt::skip]
    let buf = vec![
        0x80, 0x00, 0x00, 0x30, // head
        0x00, 0x00, 0x00, 0x01, // xid
        0x00, 0x00, 0x00, 0x02, // invalid msg type (must be CALL = 0)
        0x00, 0x00, 0x00, 0x02, // rpc version
        0x00, 0x01, 0x86, 0xA3, // program
        0x00, 0x00, 0x00, 0x03, // prog vers
        0x00, 0x00, 0x00, 0x12, // proc
        0x00, 0x00, 0x00, 0x00, // auth
        0x00, 0x00, 0x00, 0x00, // auth
        0x00, 0x00, 0x00, 0x08, // nfs_fh3
        0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
    ];
    let socket = MockSocket::new(buf.as_slice());
    let alloc = MockAllocator::new(0);
    let mut parser = RpcParser::with_capacity(socket, alloc, 0x35);

    let result = parser.parse_message().await;
    assert!(matches!(result, Err(Error::MessageTypeMismatch)));
}

/// Verifies parser handles WRITE with zero opaque payload.
#[tokio::test]
async fn parse_write_with_empty_payload() {
    #[rustfmt::skip]
    let buf = vec![
        0x80, 0x00, 0x00, 68, // head
        0x00, 0x00, 0x00, 0x01, // xid
        0x00, 0x00, 0x00, 0x00, // request
        0x00, 0x00, 0x00, 0x02, // rpc version
        0x00, 0x01, 0x86, 0xA3, // program
        0x00, 0x00, 0x00, 0x03, // prog vers
        0x00, 0x00, 0x00, 7, // proc
        0x00, 0x00, 0x00, 0x00, // auth
        0x00, 0x00, 0x00, 0x00, //auth
        0x00, 0x00, 0x00, 0x08, // nfs_fh3
        0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
        0x00, 0x00, 0x00, 0x00, // offset
        0x00, 0x00, 0x80, 0x00, // offset
        0x00, 0x00, 0x00, 0xFF, // count
        0x00, 0x00, 0x00, 0x00, // mode
        0x00, 0x00, 0x00, 0x00, // opaque length
    ];
    let socket = MockSocket::new(buf.as_slice());
    let alloc = MockAllocator::new(1);
    let mut parser = RpcParser::with_capacity(socket, alloc, 68);
    let result = parser.parse_message().await;
    assert!(result.is_ok());
}
