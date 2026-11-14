//! Limits the read task of a connection enforces: whether it may read the next request
//! and whether the connection has to close.

use std::future::pending;

use async_channel::Sender;

use crate::allocator::Buffer;
use crate::limits::budget::{Budget, InFlightReply, ReplySender};
use crate::limits::probation::Ticket;
use crate::limits::Limits;
use crate::parser::NfsArguments;

/// Why a connection has to close, see [`ConnectionLimits::expired`].
pub enum Expiry {
    /// Pushed out of probation by newer connections, see
    /// [`Limits::max_unverified_connections`].
    Evicted,
    /// Idle for too long, see [`Limits::idle_timeout`].
    Idle,
}

/// Limits of one connection, held by its read task.
pub struct ConnectionLimits<B: Buffer> {
    /// [`Limits::max_in_flight`], [`Limits::max_data_per_connection`] and
    /// [`Limits::idle_timeout`].
    budget: Budget<B>,
    /// Place on probation until the first valid call, `None` after it.
    probation: Option<Ticket>,
}

impl<B: Buffer> ConnectionLimits<B> {
    /// Creates the limits of a connection whose replies are sent through `tx`; with a
    /// `ticket` the connection starts on probation.
    pub fn new(limits: &Limits, tx: Sender<InFlightReply<B>>, ticket: Option<Ticket>) -> Self {
        Self { budget: Budget::new(limits, tx), probation: ticket }
    }

    /// Takes the slot of the next request, waiting while the connection has as many
    /// requests or as much data in flight as it may. Called before the request is read.
    pub async fn acquire(&mut self) -> ReplySender<B> {
        self.budget.acquire().await
    }

    /// Charges the READ or WRITE data `proc` holds to the request `result_tx` answers;
    /// it is freed with the slot.
    pub fn charge(&mut self, result_tx: &mut ReplySender<B>, proc: &NfsArguments<B>) {
        let bytes = match proc {
            NfsArguments::Read(_, data) => data.footprint(),
            NfsArguments::Write(args) => args.data.footprint(),
            _ => return,
        };
        self.budget.charge(result_tx, bytes);
    }

    /// Completes once the connection has to close; never while it may stay open. Called
    /// after [`Self::acquire`], while the next request is read.
    pub async fn expired(&mut self) -> Expiry {
        let evicted = async {
            match &self.probation {
                Some(ticket) => ticket.evicted().await,
                None => pending().await,
            }
        };
        tokio::select! {
            biased;
            () = evicted => Expiry::Evicted,
            () = self.budget.idle() => Expiry::Idle,
        }
    }

    /// Takes the connection off probation; called on its first valid call.
    pub fn verified(&mut self) {
        self.probation = None;
    }
}
