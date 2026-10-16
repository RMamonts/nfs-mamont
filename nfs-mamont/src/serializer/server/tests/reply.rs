//! Replies to calls rejected by the parser, as produced by [`Serializer::form_reply`].

use crate::allocator::Slice;
use crate::consts::nfsv3::{LOOKUP, WRITE};
use crate::rpc::{AcceptStat, AuthFlavor, Error, OpaqueAuth};
use crate::serializer::server::serialize_struct::Serializer;
use crate::task::ProcReply;
use crate::vfs;

fn verifier() -> OpaqueAuth {
    OpaqueAuth { flavor: AuthFlavor::None, body: vec![] }
}

/// Wire bytes of an accepted reply with an `AUTH_NONE` verifier, `stat` and `body`.
fn accepted(xid: u32, stat: AcceptStat, body: &[u32]) -> Vec<u8> {
    let mut words = vec![xid, 1, 0, 0, 0, stat as u32];
    words.extend_from_slice(body);
    let mut bytes = (0x8000_0000 | (words.len() * 4) as u32).to_be_bytes().to_vec();
    for word in words {
        bytes.extend_from_slice(&word.to_be_bytes());
    }
    bytes
}

/// Serializes the reply to call `xid` rejected with `error`.
async fn reject(xid: u32, error: Error) -> Vec<u8> {
    let mut out = Vec::new();
    let mut serializer = Serializer::new(&mut out);
    let reply = ProcReply::<Slice> { xid, proc_result: Err(error) };
    serializer.form_reply(reply, verifier()).await.unwrap();
    out
}

#[tokio::test]
async fn nfs3_status_error_is_encoded_as_procedure_failure() {
    let error = Error::Nfs3Status { procedure: LOOKUP, status: vfs::Error::BadFileHandle };

    // NFS3ERR_BADHANDLE, then LOOKUP3resfail without directory attributes.
    assert_eq!(reject(1, error).await, accepted(1, AcceptStat::Success, &[10001, 0]));
}

#[tokio::test]
async fn nfs3_status_error_carries_empty_wcc_data() {
    let error = Error::Nfs3Status { procedure: WRITE, status: vfs::Error::NameTooLong };

    // NFS3ERR_NAMETOOLONG, then WRITE3resfail with empty before/after attributes.
    assert_eq!(reject(1, error).await, accepted(1, AcceptStat::Success, &[63, 0, 0]));
}

#[tokio::test]
async fn malformed_call_is_answered_with_garbage_args() {
    let out = reject(1, Error::Malformed("trailing bytes")).await;

    assert_eq!(out, accepted(1, AcceptStat::GarbageArgs, &[]));
}
