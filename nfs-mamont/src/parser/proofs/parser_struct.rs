//! Kani harnesses for [`RpcParser`] message framing.

use std::num::NonZeroUsize;
use std::sync::Arc;

use super::socket::MockSocket;
use crate::allocator::{Allocator, Slice};
use crate::consts::nfsv3::{GETATTR, NFS3_FHSIZE, NFS_PROGRAM, NFS_VERSION};
use crate::consts::rpc::HEADER_MASK;
use crate::consts::xdr::ALIGNMENT;
use crate::parser::parser_struct::RpcParser;
use crate::parser::{ArgWrapper, ErrorWrapper, ProcArguments, RpcHeader};
use crate::rpc::{AuthFlavor, RpcBody, RPC_VERSION};
use crate::serializer;

const FIRST_XID: u32 = 1;
const SECOND_XID: u32 = 2;
/// Size of GETATTR arguments: an `nfs_fh3` (length word, handle, padding).
const GETATTR_ARGS: usize = 4 + NFS3_FHSIZE + (ALIGNMENT - NFS3_FHSIZE % ALIGNMENT) % ALIGNMENT;
/// Parser buffer size: holds one GETATTR call, not two.
const CAPACITY: usize = 64;

/// Allocator for calls that never allocate.
struct NoAllocator;

impl Allocator for NoAllocator {
    type Buffer = Slice;

    async fn allocate(&self, _size: NonZeroUsize) -> Option<Slice> {
        None
    }

    fn capacity(&self) -> NonZeroUsize {
        NonZeroUsize::MIN
    }
}

/// A complete record-marked NFSv3 call with `AUTH_NONE` credential and
/// verifier.
fn nfs_call(xid: u32, procedure: u32, args: &[u8]) -> Vec<u8> {
    let none = AuthFlavor::None as u32;
    let mut body = Vec::new();
    let header = [xid, RpcBody::Call as u32, RPC_VERSION, NFS_PROGRAM, NFS_VERSION, procedure];
    for word in header.into_iter().chain([none, 0, none, 0]) {
        serializer::u32(&mut body, word).unwrap();
    }
    body.extend_from_slice(args);

    let mut call = Vec::new();
    serializer::u32(&mut call, HEADER_MASK as u32 | body.len() as u32).unwrap();
    call.extend_from_slice(&body);
    call
}

/// Well-formed GETATTR arguments.
fn valid_getattr_args() -> Vec<u8> {
    let mut args = Vec::new();
    serializer::usize_as_u32(&mut args, NFS3_FHSIZE).unwrap();
    serializer::array(&mut args, [0u8; NFS3_FHSIZE]).unwrap();
    args
}

/// Whatever arguments a call carries, the parser stays aligned to record
/// boundaries: a well-formed call that follows it is parsed intact.
#[kani::proof]
#[kani::unwind(20)]
fn malformed_arguments_do_not_desync_the_stream() {
    let args: [u8; GETATTR_ARGS] = kani::any();
    let mut stream = nfs_call(FIRST_XID, GETATTR, &args);
    stream.extend(nfs_call(SECOND_XID, GETATTR, &valid_getattr_args()));

    let mut parser =
        RpcParser::with_capacity(MockSocket::whole(&stream), Arc::new(NoAllocator), CAPACITY);

    let first = kani::block_on(parser.next_message());
    kani::cover!(first.is_err(), "first call rejected");
    // A well-formed header always yields an xid to reply to.
    assert!(!matches!(first, Err(ErrorWrapper { xid: None, .. })));

    let second = kani::block_on(parser.next_message());
    assert!(matches!(
        second,
        Ok(ArgWrapper { header: RpcHeader { xid: SECOND_XID, .. }, proc: ProcArguments::Nfs3(_) })
    ));
}
