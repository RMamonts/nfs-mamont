//! Simulated backend response time: a sub-millisecond timer and the delay model.
//!
//! [`tokio::time::sleep`] rounds deadlines up to the next millisecond tick, so it cannot
//! simulate the 50–500 µs response times of a local SSD or of a fast network backend.
//! [`Timer`] keeps a dedicated OS thread that waits on a condition variable until the
//! earliest deadline and then wakes the sleeping futures. On Linux the thread also
//! lowers its timer slack, which brings the wake-up error from the default ~50 µs down
//! to a few microseconds.

use std::cmp::Ordering as CmpOrdering;
use std::collections::BinaryHeap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::task::{Context, Poll};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use tokio::sync::oneshot;

use crate::config::Latency;

/// Timer slack requested for the timer thread, in nanoseconds (Linux only).
#[cfg(target_os = "linux")]
const TIMER_SLACK_NS: libc::c_ulong = 1_000;

/// Seed of the jitter generator; fixed so that runs are reproducible.
const RNG_SEED: u64 = 0x5EED_1234_ABCD_0001;
/// Weyl sequence increment used by [`Model::random`] (the golden ratio).
const GOLDEN_GAMMA: u64 = 0x9E37_79B9_7F4A_7C15;

/// Deadline-ordered timer backed by a dedicated thread.
pub struct Timer {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

struct Shared {
    state: Mutex<State>,
    wakeup: Condvar,
}

struct State {
    queue: BinaryHeap<Pending>,
    next_seq: u64,
    closed: bool,
}

/// A registered sleep: earliest deadline first, FIFO among equal deadlines.
struct Pending {
    deadline: Instant,
    seq: u64,
    waker: oneshot::Sender<()>,
}

impl PartialEq for Pending {
    fn eq(&self, other: &Self) -> bool {
        self.deadline == other.deadline && self.seq == other.seq
    }
}

impl Eq for Pending {}

impl PartialOrd for Pending {
    fn partial_cmp(&self, other: &Self) -> Option<CmpOrdering> {
        Some(self.cmp(other))
    }
}

impl Ord for Pending {
    /// Reversed, so that the max-heap [`BinaryHeap`] yields the earliest deadline first.
    fn cmp(&self, other: &Self) -> CmpOrdering {
        other.deadline.cmp(&self.deadline).then_with(|| other.seq.cmp(&self.seq))
    }
}

/// Future returned by [`Timer::sleep_until`] and [`Timer::sleep`].
pub struct Sleep(oneshot::Receiver<()>);

impl Future for Sleep {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        // A dropped timer drops the sender; waking up early is the only sane reaction.
        Pin::new(&mut self.0).poll(cx).map(|_| ())
    }
}

impl Timer {
    /// Starts the timer thread.
    pub fn new() -> Self {
        let shared = Arc::new(Shared {
            state: Mutex::new(State { queue: BinaryHeap::new(), next_seq: 0, closed: false }),
            wakeup: Condvar::new(),
        });
        let worker = Arc::clone(&shared);
        let thread = thread::Builder::new()
            .name("mock-vfs-timer".into())
            .spawn(move || run(&worker))
            .expect("failed to spawn the mock-vfs timer thread");

        Timer { shared, thread: Some(thread) }
    }

    /// Returns a future that completes at `deadline` (or right away if it has passed).
    pub fn sleep_until(&self, deadline: Instant) -> Sleep {
        let (tx, rx) = oneshot::channel();
        let is_earliest = {
            let mut state = lock(&self.shared.state);
            let seq = state.next_seq;
            state.next_seq += 1;
            let is_earliest = match state.queue.peek() {
                Some(first) => deadline < first.deadline,
                None => true,
            };
            state.queue.push(Pending { deadline, seq, waker: tx });
            is_earliest
        };
        // The thread is parked until the previously earliest deadline; re-arm it only
        // when the new one is earlier, otherwise its current wait already covers it.
        if is_earliest {
            self.shared.wakeup.notify_one();
        }

        Sleep(rx)
    }

    /// Returns a future that completes after `duration`.
    pub fn sleep(&self, duration: Duration) -> Sleep {
        self.sleep_until(Instant::now() + duration)
    }
}

impl Default for Timer {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Timer {
    /// Stops the thread; every pending [`Sleep`] completes immediately.
    fn drop(&mut self) {
        lock(&self.shared.state).closed = true;
        self.shared.wakeup.notify_all();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn lock(state: &Mutex<State>) -> MutexGuard<'_, State> {
    state.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Body of the timer thread.
fn run(shared: &Shared) {
    #[cfg(target_os = "linux")]
    reduce_timer_slack();

    let mut expired = Vec::new();
    let mut state = lock(&shared.state);
    while !state.closed {
        let now = Instant::now();
        while state.queue.peek().is_some_and(|first| first.deadline <= now) {
            if let Some(pending) = state.queue.pop() {
                expired.push(pending);
            }
        }
        if !expired.is_empty() {
            // Wake the sleepers without holding the lock, then re-check the queue.
            drop(state);
            for pending in expired.drain(..) {
                let _ = pending.waker.send(());
            }
            state = lock(&shared.state);
            continue;
        }

        let timeout = state.queue.peek().map(|first| first.deadline.saturating_duration_since(now));
        state = match timeout {
            None => shared.wakeup.wait(state).unwrap_or_else(|poisoned| poisoned.into_inner()),
            Some(timeout) => {
                shared
                    .wakeup
                    .wait_timeout(state, timeout)
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .0
            }
        };
    }
    // Dropping the queue drops the senders, which wakes every remaining sleeper.
}

/// Lowers the timer slack of the calling thread so that timed waits wake up closer
/// to their deadline than the default 50 µs.
#[cfg(target_os = "linux")]
fn reduce_timer_slack() {
    // SAFETY: `prctl(PR_SET_TIMERSLACK, ns)` only changes a scheduling parameter of
    // the calling thread; it reads no memory through its arguments.
    unsafe {
        libc::prctl(libc::PR_SET_TIMERSLACK, TIMER_SLACK_NS);
    }
}

/// Computes completion deadlines according to a [`Latency`] and waits for them.
pub struct Model {
    latency: Latency,
    /// Absent when the latency is instant: the fast path never touches a timer.
    timer: Option<Timer>,
    /// Reference point of [`Self::device_free_at`].
    epoch: Instant,
    /// State of the jitter generator.
    rng: AtomicU64,
    /// Nanoseconds since [`Self::epoch`] at which the simulated device finishes the
    /// transfers already queued on it.
    device_free_at: AtomicU64,
}

impl Model {
    /// Creates the model; starts a [`Timer`] unless `latency` is instant.
    pub fn new(latency: Latency) -> Self {
        let timer = if latency.is_instant() { None } else { Some(Timer::new()) };
        Model {
            latency,
            timer,
            epoch: Instant::now(),
            rng: AtomicU64::new(RNG_SEED),
            device_free_at: AtomicU64::new(0),
        }
    }

    /// Returns the configured latency.
    pub fn latency(&self) -> &Latency {
        &self.latency
    }

    /// Returns `true` when requests complete without any delay.
    pub fn is_instant(&self) -> bool {
        self.timer.is_none()
    }

    /// Returns the instant at which a request transferring `bytes` bytes and issued at
    /// `now` completes, or [`None`] when the backend is instant.
    ///
    /// Calling it reserves time on the simulated device, so call it once per request.
    pub fn deadline(&self, bytes: u64, now: Instant) -> Option<Instant> {
        if self.latency.is_instant() {
            return None;
        }

        let delay = self.per_request_delay();
        let transfer_end = match self.latency.bandwidth {
            Some(bandwidth) if bytes > 0 => {
                self.reserve_device(now, transfer_time(bytes, bandwidth.get()))
            }
            _ => now,
        };

        Some(transfer_end + delay)
    }

    /// Waits until a request transferring `bytes` bytes is served.
    pub async fn wait(&self, bytes: u64) {
        if let Some(timer) = &self.timer {
            if let Some(deadline) = self.deadline(bytes, Instant::now()) {
                timer.sleep_until(deadline).await;
            }
        }
    }

    /// Draws `base ± jitter`, saturating at zero.
    fn per_request_delay(&self) -> Duration {
        let jitter = self.latency.jitter;
        if jitter.is_zero() {
            return self.latency.base;
        }

        let jitter_ns = jitter.as_nanos() as u64;
        let span = jitter_ns.saturating_mul(2).saturating_add(1);
        let offset = (self.random() % span) as i128 - i128::from(jitter_ns);
        let nanos = (self.latency.base.as_nanos() as i128 + offset).max(0);

        Duration::from_nanos(nanos as u64)
    }

    /// Queues a transfer taking `service` on the device and returns when it ends.
    fn reserve_device(&self, now: Instant, service: Duration) -> Instant {
        let now_ns = now.saturating_duration_since(self.epoch).as_nanos() as u64;
        let service_ns = service.as_nanos() as u64;

        let mut free_at = self.device_free_at.load(Ordering::Relaxed);
        loop {
            let end = free_at.max(now_ns).saturating_add(service_ns);
            match self.device_free_at.compare_exchange_weak(
                free_at,
                end,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => return self.epoch + Duration::from_nanos(end),
                Err(actual) => free_at = actual,
            }
        }
    }

    /// Returns a pseudo-random number; lock-free and good enough for jitter.
    fn random(&self) -> u64 {
        splitmix64(self.rng.fetch_add(GOLDEN_GAMMA, Ordering::Relaxed))
    }
}

/// Time needed to move `bytes` bytes at `bandwidth` bytes per second.
fn transfer_time(bytes: u64, bandwidth: u64) -> Duration {
    let nanos = u128::from(bytes) * 1_000_000_000 / u128::from(bandwidth);
    Duration::from_nanos(nanos.min(u128::from(u64::MAX)) as u64)
}

/// SplitMix64 finalizer: turns a counter into a well-mixed 64-bit value.
pub fn splitmix64(mut x: u64) -> u64 {
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;
    use std::time::{Duration, Instant};

    use super::{Model, Timer};
    use crate::config::Latency;

    const MIB: u64 = 1 << 20;

    #[test]
    fn instant_backend_has_no_deadline() {
        let model = Model::new(Latency::INSTANT);

        assert!(model.is_instant());
        assert!(model.deadline(MIB, Instant::now()).is_none());
    }

    #[test]
    fn fixed_latency_is_added_to_now() {
        let model = Model::new(Latency::fixed(Duration::from_millis(3)));
        let now = Instant::now();

        assert_eq!(model.deadline(0, now), Some(now + Duration::from_millis(3)));
        // Without a bandwidth limit the transfer size does not matter.
        assert_eq!(model.deadline(MIB, now), Some(now + Duration::from_millis(3)));
    }

    #[test]
    fn jitter_stays_within_bounds() {
        let latency = Latency {
            base: Duration::from_micros(500),
            jitter: Duration::from_micros(200),
            bandwidth: None,
        };
        let model = Model::new(latency);
        let now = Instant::now();

        let mut seen_below = false;
        let mut seen_above = false;
        for _ in 0..1000 {
            let deadline = model.deadline(0, now).unwrap();
            let delay = deadline - now;
            assert!(delay >= Duration::from_micros(300), "delay {delay:?} below the lower bound");
            assert!(delay <= Duration::from_micros(700), "delay {delay:?} above the upper bound");
            seen_below |= delay < Duration::from_micros(500);
            seen_above |= delay > Duration::from_micros(500);
        }
        assert!(seen_below && seen_above, "jitter never left the base latency");
    }

    #[test]
    fn jitter_saturates_at_zero() {
        let latency = Latency {
            base: Duration::from_micros(10),
            jitter: Duration::from_micros(100),
            bandwidth: None,
        };
        let model = Model::new(latency);
        let now = Instant::now();

        for _ in 0..1000 {
            assert!(model.deadline(0, now).unwrap() >= now);
        }
    }

    #[test]
    fn bandwidth_serializes_transfers_on_one_device() {
        let latency = Latency {
            base: Duration::from_millis(1),
            jitter: Duration::ZERO,
            bandwidth: NonZeroU64::new(MIB),
        };
        let model = Model::new(latency);
        let now = Instant::now();

        // Two 1 MiB transfers at 1 MiB/s issued at the same time: the second one waits
        // for the first, and both pay the per-request latency on top.
        assert_eq!(model.deadline(MIB, now), Some(now + Duration::from_millis(1001)));
        assert_eq!(model.deadline(MIB, now), Some(now + Duration::from_millis(2001)));
        // Metadata operations do not touch the device.
        assert_eq!(model.deadline(0, now), Some(now + Duration::from_millis(1)));
        // Once the device is idle again, a transfer starts right away.
        let later = now + Duration::from_secs(10);
        assert_eq!(model.deadline(MIB, later), Some(later + Duration::from_millis(1001)));
    }

    #[tokio::test]
    async fn timer_fires_after_the_deadline() {
        let timer = Timer::new();
        let delay = Duration::from_micros(300);

        let start = Instant::now();
        timer.sleep(delay).await;

        assert!(start.elapsed() >= delay);
    }

    #[tokio::test]
    async fn concurrent_sleeps_complete_in_deadline_order() {
        let timer = Timer::new();
        let start = Instant::now();

        let late = timer.sleep(Duration::from_millis(6));
        let early = timer.sleep(Duration::from_millis(2));
        let (early_done, late_done) = tokio::join!(
            async {
                early.await;
                Instant::now()
            },
            async {
                late.await;
                Instant::now()
            }
        );

        assert!(early_done - start >= Duration::from_millis(2));
        assert!(late_done - start >= Duration::from_millis(6));
        assert!(early_done <= late_done);
    }

    #[tokio::test]
    async fn dropping_the_timer_wakes_pending_sleeps() {
        let timer = Timer::new();
        let sleep = timer.sleep(Duration::from_secs(3600));
        drop(timer);

        let start = Instant::now();
        sleep.await;

        assert!(start.elapsed() < Duration::from_secs(5));
    }

    #[tokio::test]
    async fn model_wait_respects_the_latency() {
        let model = Model::new(Latency::fixed(Duration::from_millis(2)));

        let start = Instant::now();
        model.wait(0).await;

        assert!(start.elapsed() >= Duration::from_millis(2));
    }
}
