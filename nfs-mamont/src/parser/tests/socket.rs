use tokio::net::tcp::OwnedReadHalf;
use tokio::net::{TcpListener, TcpStream};

/// Returns the read half of a connected TCP pair with `bytes` written into the
/// peer, so the returned half behaves like a real socket that has already
/// received the given frame payload.
pub async fn read_half(bytes: &[u8]) -> OwnedReadHalf {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let client = TcpStream::connect(addr).await.unwrap();
    let server = listener.accept().await.unwrap().0;

    let (read_half, _write_half) = server.into_split();

    client.try_write(bytes).unwrap();
    drop(client);

    read_half
}
