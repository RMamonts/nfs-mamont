use std::future::Future;
use std::time::Duration;

pub mod net {
    pub use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
    pub use tokio::net::{TcpListener, TcpStream};
}

pub mod sync {
    pub mod mpsc {
        pub use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
    }
}

pub fn spawn<F>(future: F) -> tokio::task::JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    tokio::spawn(future)
}

pub async fn timeout<T>(duration: Duration, future: T) -> Result<T::Output, tokio::time::error::Elapsed>
where
    T: Future,
{
    tokio::time::timeout(duration, future).await
}

#[cfg(feature = "tokio-uring-runtime")]
pub fn start<F>(future: F) -> F::Output
where
    F: Future,
{
    tokio_uring::start(future)
}