use tokio::io::AsyncWriteExt;
use tokio::net::tcp::OwnedReadHalf;
use tokio::net::{TcpListener, TcpStream};

pub struct MockSocket(OwnedReadHalf);

impl MockSocket {
    const ADDRESS: &str = "127.0.0.1:0";
    pub async fn new(bytes: Vec<u8>) -> MockSocket {
        let listener = TcpListener::bind(Self::ADDRESS).await.unwrap();
        let addr = listener.local_addr().unwrap();

        let listener_task = tokio::task::spawn(async move {
            let (server, _) = listener.accept().await.unwrap();
            let (read_half, _) = server.into_split();
            read_half
        });

        let client_task = tokio::task::spawn(async move {
            let mut client = TcpStream::connect(addr).await.unwrap();
            client.write_all(&bytes).await.unwrap();
        });

        let (half, _) = tokio::join!(listener_task, client_task);
        Self(half.unwrap())
    }

    pub fn into_inner(self) -> OwnedReadHalf {
        self.0
    }
}
