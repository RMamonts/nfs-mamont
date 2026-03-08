use std::io;
use std::num::NonZeroUsize;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;
use tokio::time::timeout;

use nfs_mamont::nfsv3::{NFS_PROGRAM, NFS_VERSION};
use nfs_mamont::rpc::{RpcBody, ServerContext, ServerSettings, RPC_VERSION};

const RECORD_LAST_FRAGMENT_BIT: u32 = 0x8000_0000;
const AUTH_NONE: u32 = 0;
const NULL_PROCEDURE: u32 = 0;

fn non_zero(value: usize) -> NonZeroUsize {
    match NonZeroUsize::new(value) {
        Some(value) => value,
        None => panic!("test helper expects non-zero value"),
    }
}

fn push_u32(buf: &mut Vec<u8>, value: u32) {
    buf.extend(value.to_be_bytes());
}

fn null_call_frame(xid: u32) -> Vec<u8> {
    let mut body = Vec::new();
    push_u32(&mut body, xid);
    push_u32(&mut body, RpcBody::Call as u32);
    push_u32(&mut body, RPC_VERSION);
    push_u32(&mut body, NFS_PROGRAM);
    push_u32(&mut body, NFS_VERSION);
    push_u32(&mut body, NULL_PROCEDURE);
    push_u32(&mut body, AUTH_NONE);
    push_u32(&mut body, 0);
    push_u32(&mut body, AUTH_NONE);
    push_u32(&mut body, 0);

    let mut frame = Vec::with_capacity(body.len() + size_of::<u32>());
    push_u32(&mut frame, RECORD_LAST_FRAGMENT_BIT | body.len() as u32);
    frame.extend(body);
    frame
}

async fn read_record(stream: &mut TcpStream) -> io::Result<Vec<u8>> {
    let mut header = [0_u8; 4];
    stream.read_exact(&mut header).await?;

    let length = u32::from_be_bytes(header) & !RECORD_LAST_FRAGMENT_BIT;
    let mut payload = vec![0_u8; length as usize];
    stream.read_exact(&mut payload).await?;
    Ok(payload)
}

fn reply_xid(reply: &[u8]) -> u32 {
    u32::from_be_bytes(reply[..4].try_into().expect("reply must contain xid"))
}

async fn start_server(
    settings: ServerSettings,
) -> io::Result<(std::net::SocketAddr, JoinHandle<()>)> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let local_addr = listener.local_addr()?;
    let server = tokio::spawn(async move {
        let _ = nfs_mamont::handle_forever_with_context(
            listener,
            ServerContext::with_settings(settings),
        )
        .await;
    });
    Ok((local_addr, server))
}

#[tokio::test]
async fn server_accepts_new_connection_after_client_disconnect() {
    let (addr, server) = start_server(ServerSettings::new()).await.expect("server should start");

    let mut first_client = TcpStream::connect(addr).await.expect("first client should connect");
    first_client.write_all(&null_call_frame(1)).await.expect("first request should be written");

    let first_reply = timeout(Duration::from_secs(1), read_record(&mut first_client))
        .await
        .expect("first reply should arrive")
        .expect("first reply should be readable");
    assert_eq!(reply_xid(&first_reply), 1);

    drop(first_client);

    tokio::time::sleep(Duration::from_millis(50)).await;

    let mut second_client = TcpStream::connect(addr).await.expect("second client should connect");
    second_client.write_all(&null_call_frame(2)).await.expect("second request should be written");

    let second_reply = timeout(Duration::from_secs(1), read_record(&mut second_client))
        .await
        .expect("second reply should arrive")
        .expect("second reply should be readable");
    assert_eq!(reply_xid(&second_reply), 2);

    server.abort();
    let _ = server.await;
}

#[tokio::test]
async fn bounded_connection_queues_preserve_all_null_replies() {
    let settings = ServerSettings::new()
        .with_command_queue_size(non_zero(1))
        .with_result_queue_size(non_zero(1))
        .with_max_in_flight_requests(non_zero(2));
    let (addr, server) = start_server(settings).await.expect("server should start");

    let mut client = TcpStream::connect(addr).await.expect("client should connect");
    let request_stream = (1..=8_u32).flat_map(null_call_frame).collect::<Vec<u8>>();

    client.write_all(&request_stream).await.expect("requests should be written");

    for expected_xid in 1..=8_u32 {
        let reply = timeout(Duration::from_secs(1), read_record(&mut client))
            .await
            .expect("reply should arrive")
            .expect("reply should be readable");
        assert_eq!(reply_xid(&reply), expected_xid);
    }

    server.abort();
    let _ = server.await;
}
