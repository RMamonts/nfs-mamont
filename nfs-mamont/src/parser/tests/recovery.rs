//! Rejected calls must not misalign the stream: whatever is wrong with a call,
//! the parser skips the rest of its frame and parses the next one.

use std::io::ErrorKind;
use std::sync::Arc;

use crate::consts::nfsv3::{
    CREATE, GETATTR, LOOKUP, MKNOD, NFS_PROGRAM, NFS_VERSION, NULL, READ, WRITE,
};
use crate::parser::parser_struct::{RpcParser, MAX_RECORD_SIZE};
use crate::parser::tests::allocator::MockAllocator;
use crate::parser::tests::socket::MockSocket;
use crate::parser::{Error, ErrorWrapper, NfsArguments, ProcArguments};
use crate::rpc::AuthFlavor;

/// Capacity of the parser buffer; smaller than some of the frames below, so
/// skipping their rest also exercises reads straight from the socket.
const CAPACITY: usize = 0x100;

/// Allocator limit; WRITE payloads above it cannot be allocated.
const ALLOC_MAX: usize = 64;

/// An `opaque_auth` as `(flavor, body)`.
type Auth<'a> = (u32, &'a [u8]);

const AUTH_NONE: Auth = (AuthFlavor::None as u32, &[]);

fn push_u32(buf: &mut Vec<u8>, value: u32) {
    buf.extend_from_slice(&value.to_be_bytes());
}

fn push_u64(buf: &mut Vec<u8>, value: u64) {
    buf.extend_from_slice(&value.to_be_bytes());
}

fn push_opaque(buf: &mut Vec<u8>, bytes: &[u8]) {
    push_u32(buf, bytes.len() as u32);
    buf.extend_from_slice(bytes);
    buf.resize(buf.len() + (4 - bytes.len() % 4) % 4, 0);
}

/// Builds a record-marked NFSv3 CALL frame with raw procedure arguments.
fn call(xid: u32, procedure: u32, cred: Auth, verf: Auth, args: &[u8]) -> Vec<u8> {
    let mut body = Vec::new();
    for word in [xid, 0, 2, NFS_PROGRAM, NFS_VERSION, procedure, cred.0] {
        push_u32(&mut body, word);
    }
    push_opaque(&mut body, cred.1);
    push_u32(&mut body, verf.0);
    push_opaque(&mut body, verf.1);
    body.extend_from_slice(args);

    let mut frame = Vec::new();
    push_u32(&mut frame, 0x8000_0000 | body.len() as u32);
    frame.extend(body);
    frame
}

fn nfs_call(xid: u32, procedure: u32, args: &[u8]) -> Vec<u8> {
    call(xid, procedure, AUTH_NONE, AUTH_NONE, args)
}

/// Encodes an `nfs_fh3` of `size` bytes.
fn handle(size: usize) -> Vec<u8> {
    let mut buf = Vec::new();
    push_opaque(&mut buf, &vec![0x01; size]);
    buf
}

/// Encodes `diropargs3` with a valid handle and `name`.
fn dir_op(name: &[u8]) -> Vec<u8> {
    let mut buf = handle(9);
    push_opaque(&mut buf, name);
    buf
}

/// Encodes WRITE arguments with an explicit `count` and opaque `data`.
fn write_args(fh: &[u8], count: u32, data: &[u8]) -> Vec<u8> {
    let mut buf = fh.to_vec();
    push_u64(&mut buf, 0);
    push_u32(&mut buf, count);
    push_u32(&mut buf, 0);
    push_opaque(&mut buf, data);
    buf
}

/// Parses `first` (xid 1) followed by an NFS NULL call (xid 2).
///
/// Checks that `first` is rejected and that the NULL call after it still parses,
/// and returns the error `first` was rejected with.
async fn reject_then_recover(first: Vec<u8>) -> Error {
    let mut stream = first;
    stream.extend(nfs_call(2, NULL, &[]));
    let socket = MockSocket::new(&stream);
    let alloc = Arc::new(MockAllocator::new(ALLOC_MAX));
    let mut parser = RpcParser::with_capacity(socket, alloc, CAPACITY);

    let error = match parser.next_message().await {
        Err(ErrorWrapper { xid: Some(1), error }) => error,
        Err(other) => panic!("unexpected rejection: {other:?}"),
        Ok(_) => panic!("the first call must be rejected"),
    };

    let next = parser.next_message().await.unwrap();
    assert_eq!(next.header.xid, 2);
    assert!(matches!(next.proc, ProcArguments::Nfs3(args) if matches!(*args, NfsArguments::Null)));
    error
}

#[tokio::test]
async fn name_that_is_not_utf8_followed_by_more_arguments() {
    let mut args = dir_op(b"caf\xe9.txt");
    push_u32(&mut args, 0); // UNCHECKED
    for _ in 0..6 {
        push_u32(&mut args, 0); // sattr3 that changes nothing
    }

    let error = reject_then_recover(nfs_call(1, CREATE, &args)).await;

    assert!(matches!(error, Error::IncorrectString(_)));
}

#[tokio::test]
async fn handle_of_foreign_size() {
    let error = reject_then_recover(nfs_call(1, GETATTR, &handle(32))).await;

    assert!(matches!(error, Error::BadFileHandle));
}

#[tokio::test]
async fn read_with_foreign_handle() {
    let mut args = handle(16);
    push_u64(&mut args, 0);
    push_u32(&mut args, 4096);

    let error = reject_then_recover(nfs_call(1, READ, &args)).await;

    assert!(matches!(error, Error::BadFileHandle));
}

#[tokio::test]
async fn write_with_foreign_handle_skips_its_payload() {
    let args = write_args(&handle(16), 300, &[0xAB; 300]);

    let error = reject_then_recover(nfs_call(1, WRITE, &args)).await;

    assert!(matches!(error, Error::BadFileHandle));
}

#[tokio::test]
async fn write_that_cannot_be_allocated_skips_its_payload() {
    let args = write_args(&handle(9), 300, &[0xAB; 300]);

    let error = reject_then_recover(nfs_call(1, WRITE, &args)).await;

    assert!(matches!(error, Error::IO(err) if err.kind() == ErrorKind::OutOfMemory));
}

#[tokio::test]
async fn name_longer_than_server_limit() {
    let error = reject_then_recover(nfs_call(1, LOOKUP, &dir_op(&[b'a'; 256]))).await;

    assert!(matches!(error, Error::MaxElemLimit));
}

#[tokio::test]
async fn mknod_of_regular_file() {
    let mut args = dir_op(b"file");
    push_u32(&mut args, 1); // NF3REG

    let error = reject_then_recover(nfs_call(1, MKNOD, &args)).await;

    assert!(matches!(error, Error::EnumDiscMismatch));
}

#[tokio::test]
async fn trailing_bytes() {
    let mut args = handle(9);
    push_u64(&mut args, 0xDEAD_BEEF_DEAD_BEEF);

    let error = reject_then_recover(nfs_call(1, GETATTR, &args)).await;

    assert!(matches!(error, Error::IO(err) if err.kind() == ErrorKind::InvalidData));
}

#[tokio::test]
async fn unknown_credential_flavor() {
    // AUTH_TLS, sent by Linux clients mounting with `xprtsec=tls`.
    let frame = call(1, GETATTR, (7, &[]), AUTH_NONE, &handle(9));

    let error = reject_then_recover(frame).await;

    assert!(matches!(error, Error::EnumDiscMismatch));
}

#[tokio::test]
async fn oversized_verifier() {
    let frame = call(1, GETATTR, AUTH_NONE, (0, &[0; 404]), &handle(9));

    let error = reject_then_recover(frame).await;

    assert!(matches!(error, Error::MaxElemLimit));
}

#[tokio::test]
async fn record_larger_than_any_call_is_not_read() {
    let mut stream = Vec::new();
    push_u32(&mut stream, 0x8000_0000 | (MAX_RECORD_SIZE + 4) as u32);
    stream.extend(nfs_call(2, NULL, &[]));
    let socket = MockSocket::new(&stream);
    let mut parser = RpcParser::with_capacity(socket, Arc::new(MockAllocator::new(0)), CAPACITY);

    let result = parser.next_message().await;

    assert!(matches!(
        result,
        Err(ErrorWrapper { xid: None, error: Error::IO(err) }) if err.kind() == ErrorKind::InvalidData
    ));
}
