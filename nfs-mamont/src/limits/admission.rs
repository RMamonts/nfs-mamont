//! Admission of new connections: [`Limits::max_connections`] and
//! [`Limits::max_unverified_connections`].

use std::io;
use std::sync::Arc;

use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::limits::probation::{Probation, Ticket};
use crate::limits::Limits;

/// Lets new connections in.
pub struct Admission {
    /// Slots of [`Limits::max_connections`], `None` for no limit.
    slots: Option<Arc<Semaphore>>,
    probation: Probation,
}

/// What an admitted connection holds until it closes.
pub struct Pass {
    /// Slot of the connection in [`Limits::max_connections`], given back when dropped.
    pub slot: Option<OwnedSemaphorePermit>,
    /// Place of the connection on probation, given up with its first valid call.
    pub ticket: Ticket,
}

impl Admission {
    /// Creates the admission of a server with `limits`; no connection is admitted yet.
    pub fn new(limits: &Limits) -> Self {
        // `Semaphore::new` panics above `MAX_PERMITS`.
        let slots = limits
            .max_connections
            .map(|max| Arc::new(Semaphore::new(max.get().min(Semaphore::MAX_PERMITS))));
        Self { slots, probation: Probation::new(limits.max_unverified_connections) }
    }

    /// Accepts the next connection and puts it on probation.
    ///
    /// At the connection limit the oldest connection on probation is closed to make
    /// room; without one, the new connection waits until another one is closed, and the
    /// connections behind it wait in the kernel's accept queue.
    pub async fn accept(&self, listener: &TcpListener) -> io::Result<(TcpStream, Pass)> {
        let (socket, _) = listener.accept().await?;
        let slot = match &self.slots {
            Some(slots) => {
                if slots.available_permits() == 0 {
                    self.probation.evict_oldest();
                }
                Arc::clone(slots).acquire_owned().await.ok()
            }
            None => None,
        };
        Ok((socket, Pass { slot, ticket: self.probation.admit() }))
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;
    use std::time::Duration;

    use tokio::net::{TcpListener, TcpStream};
    use tokio::time::timeout;

    use super::Admission;
    use crate::limits::{self, Limits};

    const WAIT: Duration = Duration::from_secs(5);

    /// A listener with three clients waiting to be accepted, and an admission of one
    /// connection.
    async fn listener() -> (TcpListener, Vec<TcpStream>, Admission) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let mut clients = Vec::new();
        for _ in 0..3 {
            clients.push(TcpStream::connect(addr).await.unwrap());
        }
        let limits =
            Limits { max_connections: NonZeroUsize::new(1), ..limits::defaults(NonZeroUsize::MIN) };
        (listener, clients, Admission::new(&limits))
    }

    #[tokio::test]
    async fn accept_waits_for_a_free_connection_slot() {
        let (listener, _clients, admission) = listener().await;

        let (_first, pass) = timeout(WAIT, admission.accept(&listener)).await.unwrap().unwrap();
        // Sent no call yet, but has left probation, so it cannot be closed to make room.
        drop(pass.ticket);
        // Cancelling the wait drops the second connection; the third one is accepted below.
        let second = admission.accept(&listener);
        assert!(
            timeout(Duration::from_millis(50), second).await.is_err(),
            "accepted past the limit"
        );

        drop(pass.slot);
        timeout(WAIT, admission.accept(&listener)).await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn at_the_limit_a_connection_that_sent_nothing_makes_room() {
        let (listener, _clients, admission) = listener().await;
        let (_first, pass) = timeout(WAIT, admission.accept(&listener)).await.unwrap().unwrap();

        // What the first connection does: it closes, giving its slot back, once pushed out.
        tokio::spawn(async move {
            pass.ticket.evicted().await;
            drop(pass.slot);
        });

        timeout(WAIT, admission.accept(&listener)).await.unwrap().unwrap();
    }
}
