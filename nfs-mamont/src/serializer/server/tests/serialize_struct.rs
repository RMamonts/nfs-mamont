use tokio::io::AsyncReadExt;

use crate::allocator::{Slice, UnownedBuffer};
use crate::rpc::{AuthFlavor, OpaqueAuth};
use crate::serializer::server::serialize_struct::Serializer;
use crate::task::{ProcReply, ProcResult};
use crate::vfs::{read, NfsRes};

/// Mask of the "last fragment" bit in an RMS header.
const FRAGMENT_HEADER_MASK: u32 = 0x8000_0000;

fn verifier() -> OpaqueAuth {
    OpaqueAuth { flavor: AuthFlavor::None, body: vec![] }
}

fn null_reply(xid: u32) -> ProcReply<Slice> {
    ProcReply { xid, proc_result: Ok(ProcResult::Nfs3(Box::new(NfsRes::Null))) }
}

/// Walks RMS frames in `stream` and returns the byte ranges of their bodies.
fn split_frames(stream: &[u8]) -> Vec<&[u8]> {
    let mut frames = Vec::new();
    let mut offset = 0;
    while offset < stream.len() {
        let header = u32::from_be_bytes(stream[offset..offset + 4].try_into().unwrap());
        assert!(header & FRAGMENT_HEADER_MASK != 0, "last fragment bit must be set");
        let size = (header & !FRAGMENT_HEADER_MASK) as usize;
        offset += 4;
        frames.push(&stream[offset..offset + size]);
        offset += size;
    }
    assert_eq!(offset, stream.len(), "stream must contain whole frames only");
    frames
}

/// Test: several replies staged in one batch produce exactly the same byte
/// stream as the same replies flushed one by one.
#[tokio::test]
async fn batched_replies_match_sequential() {
    let (tx, mut rx) = tokio::io::duplex(1 << 20);
    let mut serializer = Serializer::<Slice, _>::new(tx);
    for xid in [1u32, 2, 3] {
        serializer.form_reply(null_reply(xid), verifier()).await.unwrap();
    }
    serializer.flush().await.unwrap();
    drop(serializer);
    let mut batched = Vec::new();
    rx.read_to_end(&mut batched).await.unwrap();

    let (tx, mut rx) = tokio::io::duplex(1 << 20);
    let mut serializer = Serializer::<Slice, _>::new(tx);
    let mut sequential = Vec::new();
    for xid in [1u32, 2, 3] {
        serializer.form_reply(null_reply(xid), verifier()).await.unwrap();
        serializer.flush().await.unwrap();
    }
    drop(serializer);
    rx.read_to_end(&mut sequential).await.unwrap();

    assert_eq!(batched, sequential);
    assert_eq!(split_frames(&batched).len(), 3);
}

/// Test: a successful READ reply flushes the staged batch together with its
/// payload; the payload and its XDR padding land at the tail of the frame.
#[tokio::test]
async fn read_payload_flushes_staged_batch() {
    let data: Vec<u8> = (1..=10u8).collect();
    let padding = 2; // 10 % 4 == 2, XDR pads to 12
    let mut backing = data.clone();
    let buffer = unsafe { UnownedBuffer::from_raw_parts(backing.as_mut_ptr(), backing.len()) };
    let slice = Slice::new(vec![buffer], 0..data.len(), None);

    let (tx, mut rx) = tokio::io::duplex(1 << 20);
    let mut serializer = Serializer::<Slice, _>::new(tx);
    serializer.form_reply(null_reply(7), verifier()).await.unwrap();
    assert!(serializer.buffered_len() > 0, "NULL reply must stay staged");

    let success = read::Success {
        head: read::SuccessPartial { file_attr: None, count: data.len() as u32, eof: true },
        data: slice,
    };
    let reply = ProcReply {
        xid: 8,
        proc_result: Ok(ProcResult::Nfs3(Box::new(NfsRes::Read(Ok(success))))),
    };
    serializer.form_reply(reply, verifier()).await.unwrap();
    assert_eq!(serializer.buffered_len(), 0, "payload path must flush the whole batch");

    drop(serializer);
    let mut stream = Vec::new();
    rx.read_to_end(&mut stream).await.unwrap();

    let frames = split_frames(&stream);
    assert_eq!(frames.len(), 2);
    let read_frame = frames[1];
    let tail = &read_frame[read_frame.len() - data.len() - padding..];
    assert_eq!(&tail[..data.len()], data.as_slice());
    assert!(tail[data.len()..].iter().all(|byte| *byte == 0), "padding must be zeroed");
}
