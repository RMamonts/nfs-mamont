use tokio::time::timeout;

use super::*;

fn config() -> ServerConfig {
    ServerConfig { register_rpcbind: false, handle_ctrl_c: false, ..ServerConfig::default() }
}

async fn running(state: &mut watch::Receiver<ServerState>) {
    timeout(Duration::from_secs(5), async {
        loop {
            if *state.borrow_and_update() == ServerState::Running {
                return;
            }
            state.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn accepts_connections_then_stops_on_command() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (commands, receiver) = async_channel::bounded(1);
    let (state, mut observed) = watch::channel(ServerState::Ready);
    let (accepted, connection) = async_channel::bounded(1);
    let task = tokio::spawn(run_listener(listener, config(), receiver, state, move |socket| {
        accepted.try_send(socket.peer_addr().unwrap()).unwrap();
    }));
    running(&mut observed).await;
    let client = TcpStream::connect(address).await.unwrap();
    assert_eq!(
        timeout(Duration::from_secs(5), connection.recv()).await.unwrap().unwrap(),
        client.local_addr().unwrap()
    );
    commands.send(ServerCommand::Shutdown).await.unwrap();
    timeout(Duration::from_secs(5), task).await.unwrap().unwrap().unwrap();
    assert_eq!(*observed.borrow(), ServerState::Stopped);
    assert!(TcpStream::connect(address).await.is_err());
}

#[tokio::test]
async fn dropping_last_sender_stops_idle_server() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (commands, receiver) = async_channel::bounded(1);
    let (state, mut observed) = watch::channel(ServerState::Ready);
    let task = tokio::spawn(run_listener(listener, config(), receiver, state, |_| {
        panic!("no connections expected");
    }));
    running(&mut observed).await;
    let another_sender = commands.clone();
    drop(commands);
    assert_eq!(*observed.borrow(), ServerState::Running);
    drop(another_sender);
    timeout(Duration::from_secs(5), task).await.unwrap().unwrap().unwrap();
    assert_eq!(*observed.borrow(), ServerState::Stopped);
}

#[tokio::test]
async fn shutdown_queued_before_run_is_processed() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (commands, receiver) = async_channel::bounded(1);
    commands.send(ServerCommand::Shutdown).await.unwrap();
    let (state, observed) = watch::channel(ServerState::Ready);
    let result = timeout(
        Duration::from_secs(5),
        run_listener(listener, config(), receiver, state, |_| {
            panic!("no connections expected");
        }),
    )
    .await
    .unwrap();
    assert!(result.is_ok());
    assert_eq!(*observed.borrow(), ServerState::Stopped);
}

#[test]
fn only_unusable_listener_errors_are_fatal() {
    for errno in [ACCEPT_ERROR_EBADF, ACCEPT_ERROR_EINVAL, ACCEPT_ERROR_ENOTSOCK] {
        assert!(is_fatal_accept_error(&io::Error::from_raw_os_error(errno)));
    }
    assert!(!is_fatal_accept_error(&io::Error::from_raw_os_error(24)));
    assert!(!is_fatal_accept_error(&io::Error::new(io::ErrorKind::Interrupted, "interrupted")));
}
