//! Alternative in-flight limiters compared against the branch's `Budget`.

use std::future::Future;
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use tokio::sync::{Notify, OwnedSemaphorePermit, Semaphore};

use crate::budget;

/// Common interface: the read side acquires, the write side releases.
pub trait Limit: Send + 'static {
    type Token: Send + 'static;
    type Releaser: Send + Sync + 'static;
    const NAME: &'static str;

    fn create(limit: usize) -> (Self, Arc<Self::Releaser>)
    where
        Self: Sized;
    fn acquire(&mut self) -> impl Future<Output = Self::Token> + Send;
    fn release(releaser: &Self::Releaser, token: Self::Token);
}

/// No limit at all: the baseline (what `main` does).
pub struct NoLimit;
impl Limit for NoLimit {
    type Token = ();
    type Releaser = ();
    const NAME: &'static str = "none(main)";
    fn create(_: usize) -> (Self, Arc<()>) {
        (NoLimit, Arc::new(()))
    }
    async fn acquire(&mut self) {}
    fn release(_: &(), _: ()) {}
}

/// The branch implementation, verbatim.
pub struct Branch(budget::Budget);
impl Limit for Branch {
    type Token = budget::Token;
    type Releaser = budget::Limiter;
    const NAME: &'static str = "branch Budget";
    fn create(limit: usize) -> (Self, Arc<budget::Limiter>) {
        let (b, l) = budget::Budget::new(NonZeroUsize::new(limit).unwrap());
        (Branch(b), l)
    }
    async fn acquire(&mut self) -> budget::Token {
        self.0.acquire().await.unwrap()
    }
    fn release(l: &budget::Limiter, t: budget::Token) {
        l.release(t)
    }
}

/// tokio Semaphore used like the allocator does: permit forgotten, add_permits on release.
pub struct SemForget(Arc<Semaphore>);
impl Limit for SemForget {
    type Token = ();
    type Releaser = Semaphore;
    const NAME: &'static str = "Semaphore+forget";
    fn create(limit: usize) -> (Self, Arc<Semaphore>) {
        let s = Arc::new(Semaphore::new(limit));
        (SemForget(Arc::clone(&s)), s)
    }
    async fn acquire(&mut self) {
        self.0.acquire().await.unwrap().forget();
    }
    fn release(s: &Semaphore, _: ()) {
        s.add_permits(1)
    }
}

/// tokio Semaphore with an owned RAII permit travelling with the request.
pub struct SemOwned(Arc<Semaphore>);
impl Limit for SemOwned {
    type Token = OwnedSemaphorePermit;
    type Releaser = ();
    const NAME: &'static str = "Semaphore owned(RAII)";
    fn create(limit: usize) -> (Self, Arc<()>) {
        (SemOwned(Arc::new(Semaphore::new(limit))), Arc::new(()))
    }
    async fn acquire(&mut self) -> OwnedSemaphorePermit {
        Arc::clone(&self.0).acquire_owned().await.unwrap()
    }
    fn release(_: &(), t: OwnedSemaphorePermit) {
        drop(t)
    }
}

/// The branch protocol, but the token frees its slot on drop (holds an `Arc`).
pub struct RaiiBudget {
    inner: budget::Budget,
    limiter: Arc<budget::Limiter>,
}
pub struct RaiiToken {
    token: Option<budget::Token>,
    limiter: Arc<budget::Limiter>,
}
impl Drop for RaiiToken {
    fn drop(&mut self) {
        if let Some(t) = self.token.take() {
            self.limiter.release(t);
        }
    }
}
impl Limit for RaiiBudget {
    type Token = RaiiToken;
    type Releaser = ();
    const NAME: &'static str = "Budget RAII(Arc)";
    fn create(limit: usize) -> (Self, Arc<()>) {
        let (b, l) = budget::Budget::new(NonZeroUsize::new(limit).unwrap());
        (RaiiBudget { inner: b, limiter: l }, Arc::new(()))
    }
    async fn acquire(&mut self) -> RaiiToken {
        let t = self.inner.acquire().await.unwrap();
        RaiiToken { token: Some(t), limiter: Arc::clone(&self.limiter) }
    }
    fn release(_: &(), t: RaiiToken) {
        drop(t)
    }
}

/// Same counters as the branch, but the read task asks to be woken only once
/// `batch` slots are free (hysteresis), so a saturated connection parks once per
/// `batch` requests instead of once per request.
pub struct BatchLimiter {
    released: AtomicU64,
    /// `released` value at which the parked read task wants a wake-up, `u64::MAX` if none.
    wake_at: AtomicU64,
    closed: AtomicBool,
    notify: Notify,
}
pub struct BatchBudget {
    limiter: Arc<BatchLimiter>,
    limit: u64,
    batch: u64,
    issued: u64,
    released: u64,
}
pub struct BatchToken(());
impl BatchBudget {
    fn has_slot(&self) -> bool {
        self.issued - self.released < self.limit
    }
    fn refresh(&mut self) {
        self.released = self.limiter.released.load(Ordering::SeqCst);
    }
}
impl Limit for BatchBudget {
    type Token = BatchToken;
    type Releaser = BatchLimiter;
    const NAME: &'static str = "Budget batch-wake";
    fn create(limit: usize) -> (Self, Arc<BatchLimiter>) {
        let l = Arc::new(BatchLimiter {
            released: AtomicU64::new(0),
            wake_at: AtomicU64::new(u64::MAX),
            closed: AtomicBool::new(false),
            notify: Notify::new(),
        });
        let limit = limit as u64;
        let batch = (limit / 4).clamp(1, 32);
        (BatchBudget { limiter: Arc::clone(&l), limit, batch, issued: 0, released: 0 }, l)
    }
    async fn acquire(&mut self) -> BatchToken {
        loop {
            if self.has_slot() {
                break;
            }
            self.refresh();
            if self.has_slot() {
                break;
            }
            // wake me when `batch` slots are free
            let target = self.issued - self.limit + self.batch;
            self.limiter.wake_at.store(target, Ordering::SeqCst);
            self.refresh();
            if !self.has_slot() && !self.limiter.closed.load(Ordering::SeqCst) {
                // A single free slot is not worth a park; wait for the batch.
                self.limiter.notify.notified().await;
            }
            self.limiter.wake_at.store(u64::MAX, Ordering::SeqCst);
        }
        self.issued += 1;
        BatchToken(())
    }
    fn release(l: &BatchLimiter, t: BatchToken) {
        let BatchToken(()) = t;
        let now = l.released.fetch_add(1, Ordering::SeqCst) + 1;
        if now >= l.wake_at.load(Ordering::SeqCst) {
            l.notify.notify_one();
        }
    }
}
