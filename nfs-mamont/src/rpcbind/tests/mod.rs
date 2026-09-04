use std::io::Cursor;

use byteorder::{BigEndian, WriteBytesExt};
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;

use crate::rpc::{AcceptStat, AuthFlavor, ReplyBody, RpcBody};

use super::{
    encode_call, send_rpc_call_to, server_mappings, usize_to_u32, verify_reply, Mapping,
    IPPROTO_TCP, LAST_FRAG, MOUNT_PROGRAM, MOUNT_VERSION, NFS_PROGRAM, NFS_VERSION, NLM_PROGRAM,
    NLM_VERSION, PMAP_PROC_SET, PMAP_PROGRAM, PMAP_VERSION,
};

const XID: u32 = 0x1234_5678;

#[test]
fn test_encode_call_mapping() {
    let mapping = Mapping::new(NFS_PROGRAM, NFS_VERSION, 2049);
    let record = encode_call(mapping, XID, PMAP_PROC_SET).unwrap();

    let marker = u32::from_be_bytes(record[0..4].try_into().unwrap());
    assert_eq!(marker & LAST_FRAG, LAST_FRAG, "marker must set the last-fragment bit");
    assert_eq!((marker & !LAST_FRAG) as usize, record.len() - 4);

    let mut words = record[4..].chunks_exact(4).map(|c| u32::from_be_bytes(c.try_into().unwrap()));
    let expected: &[u32] = &[
        XID,
        RpcBody::Call as u32,
        crate::rpc::RPC_VERSION,
        PMAP_PROGRAM,
        PMAP_VERSION,
        PMAP_PROC_SET,
        AuthFlavor::None as u32,
        0,
        AuthFlavor::None as u32,
        0,
        NFS_PROGRAM,
        NFS_VERSION,
        IPPROTO_TCP,
        2049,
    ];
    for want in expected {
        assert_eq!(words.next(), Some(*want), "field mismatch");
    }
    assert_eq!(words.next(), None, "unexpected trailing fields");
}

#[test]
fn test_server_mappings() {
    let port = 2049;
    assert_eq!(
        server_mappings(port),
        [
            Mapping::new(NFS_PROGRAM, NFS_VERSION, port),
            Mapping::new(MOUNT_PROGRAM, MOUNT_VERSION, port),
            Mapping::new(NLM_PROGRAM, NLM_VERSION, port)
        ]
    );
}

fn fake_reply(xid: u32, accept_success: bool, result_true: bool) -> Vec<u8> {
    let mut msg = Cursor::new(Vec::with_capacity(32));
    WriteBytesExt::write_u32::<BigEndian>(&mut msg, xid).unwrap();
    WriteBytesExt::write_u32::<BigEndian>(&mut msg, RpcBody::Reply as u32).unwrap();
    WriteBytesExt::write_u32::<BigEndian>(&mut msg, ReplyBody::MsgAccepted as u32).unwrap();
    WriteBytesExt::write_u32::<BigEndian>(&mut msg, AuthFlavor::None as u32).unwrap();
    WriteBytesExt::write_u32::<BigEndian>(&mut msg, 0).unwrap();
    WriteBytesExt::write_u32::<BigEndian>(
        &mut msg,
        if accept_success { AcceptStat::Success as u32 } else { AcceptStat::SystemErr as u32 },
    )
    .unwrap();
    WriteBytesExt::write_u32::<BigEndian>(&mut msg, if result_true { 1 } else { 0 }).unwrap();
    let msg = msg.into_inner();

    let mut record = Vec::with_capacity(msg.len() + 4);
    WriteBytesExt::write_u32::<BigEndian>(
        &mut record,
        LAST_FRAG | usize_to_u32(msg.len()).unwrap(),
    )
    .unwrap();
    record.extend_from_slice(&msg);
    record
}

#[test]
fn test_decode_success_reply() {
    let record = fake_reply(0xdead_beef, true, true);
    assert!(verify_reply(&record[4..], 0xdead_beef).is_ok());
}

#[test]
fn test_decode_wrong_xid_rejected() {
    let record = fake_reply(0xdead_beef, true, true);
    assert!(verify_reply(&record[4..], 0x1234_5678).is_err());
}

#[test]
fn test_decode_false_result_rejected() {
    let record = fake_reply(0xdead_beef, true, false);
    assert!(verify_reply(&record[4..], 0xdead_beef).is_err());
}

#[test]
fn test_decode_rejected_accept_stat_rejected() {
    let record = fake_reply(0xdead_beef, false, true);
    assert!(verify_reply(&record[4..], 0xdead_beef).is_err());
}

#[test]
fn test_fake_reply_sets_last_frag_if_parsing_ok() {
    let record = fake_reply(0xdead_beef, true, true);
    let marker = u32::from_be_bytes(record[0..4].try_into().unwrap());
    assert_eq!(marker & LAST_FRAG, LAST_FRAG, "reply marker must set last-fragment bit");
}

#[tokio::test]
async fn test_send_rpc_call_to_accepts_bit_set_reply() {
    // A fake rpcbind answering exactly as the real one (libtirpc) does:
    // the reply record marker sets LAST_FRAG. Prior to the fix the client
    // misread the length as ~2^31 and hung waiting for a non-existent body.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server_addr = listener.local_addr().unwrap();

    let server = tokio::spawn(async move {
        let (mut sock, _) = listener.accept().await.unwrap();
        let mut marker = [0u8; 4];
        sock.read_exact(&mut marker).await.unwrap();
        let marker = u32::from_be_bytes(marker);
        let len = (marker & !LAST_FRAG) as usize;
        let mut body = vec![0u8; len];
        sock.read_exact(&mut body).await.unwrap();
        let xid = u32::from_be_bytes(body[0..4].try_into().unwrap());
        let reply = fake_reply(xid, true, true);
        sock.write_all(&reply).await.unwrap();
    });

    let mapping = Mapping::new(NFS_PROGRAM, NFS_VERSION, 2049);
    let result = send_rpc_call_to(server_addr, mapping, PMAP_PROC_SET).await;
    assert!(result.is_ok(), "bit-set reply should be accepted");

    server.await.unwrap();
}

#[tokio::test]
async fn test_send_rpc_call_to_rejects_fragmented_reply() {
    // A reply whose marker omits LAST_FRAG is not a valid single-fragment
    // record; it must be rejected rather than misread as a huge record.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server_addr = listener.local_addr().unwrap();

    let server = tokio::spawn(async move {
        let (mut sock, _) = listener.accept().await.unwrap();
        let mut marker = [0u8; 4];
        sock.read_exact(&mut marker).await.unwrap();
        let marker = u32::from_be_bytes(marker);
        let len = (marker & !LAST_FRAG) as usize;
        let mut body = vec![0u8; len];
        sock.read_exact(&mut body).await.unwrap();
        let xid = u32::from_be_bytes(body[0..4].try_into().unwrap());

        let inner = fake_reply(xid, true, true);
        let mut record = inner.clone();
        record[0..4].copy_from_slice(&(inner.len() as u32 - 4).to_be_bytes());
        sock.write_all(&record).await.unwrap();
    });

    let mapping = Mapping::new(NFS_PROGRAM, NFS_VERSION, 2049);
    let result = send_rpc_call_to(server_addr, mapping, PMAP_PROC_SET).await;
    assert!(result.is_err(), "reply without last-fragment bit should be rejected");

    server.await.unwrap();
}

#[tokio::test]
async fn test_register_round_trip() {
    // A fake rpcbind that validates the SET call body and answers success.
    // The client is driven end-to-end through the real send_rpc_call_to
    // wiring.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server_addr = listener.local_addr().unwrap();
    let port = server_addr.port();

    let server = tokio::spawn(serve_one_set(listener, port));

    let mapping = Mapping::new(NFS_PROGRAM, NFS_VERSION, port);
    let result = send_rpc_call_to(server_addr, mapping, PMAP_PROC_SET).await;
    assert!(result.is_ok(), "set should succeed against the fake rpcbind");

    server.await.unwrap();
}

async fn serve_one_set(listener: TcpListener, expected_port: u16) {
    let (mut sock, _) = listener.accept().await.unwrap();
    let mut marker = [0u8; 4];
    sock.read_exact(&mut marker).await.unwrap();
    let marker = u32::from_be_bytes(marker);
    assert_eq!(marker & LAST_FRAG, LAST_FRAG, "request record must set last-fragment bit");
    let len = (marker & !LAST_FRAG) as usize;
    let mut body = vec![0u8; len];
    sock.read_exact(&mut body).await.unwrap();

    // Verify program/procedure/mapping of the received SET call.
    // Layout: [xid][msg_type][rpcvers][prog][vers][proc][cred][verf][mapping...]
    let xid = u32::from_be_bytes(body[0..4].try_into().unwrap());
    assert_eq!(&body[12..16], &PMAP_PROGRAM.to_be_bytes(), "expected portmapper prog");
    assert_eq!(&body[16..20], &PMAP_VERSION.to_be_bytes(), "expected portmapper version");
    assert_eq!(&body[20..24], &PMAP_PROC_SET.to_be_bytes(), "expected SET proc");
    assert_eq!(&body[40..44], &NFS_PROGRAM.to_be_bytes(), "expected NFS mapping prog");
    assert_eq!(&body[48..52], &IPPROTO_TCP.to_be_bytes(), "expected TCP transport");
    assert_eq!(&body[52..56], &u32::from(expected_port).to_be_bytes(), "expected requested port");

    let reply = fake_reply(xid, true, true);
    sock.write_all(&reply).await.unwrap();
}
