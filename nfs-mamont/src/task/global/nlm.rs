//! NLMv4 task dispatcher.
//!
//! Runs a background task that receives parsed NLM procedure calls from
//! connection read tasks, forwards them to the [`Nlm`] service, and sends
//! the serialized reply back to the appropriate write task.

use async_channel::{Receiver, Sender};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use tracing::debug;

use crate::allocator::Buffer;
use crate::nlm::cookie::Cookie;
use crate::nlm::lock::Nlm4Lock;
use crate::nlm::procedures::cancel::{Nlm4CancelArgs, Nlm4CancelRes};
use crate::nlm::procedures::lock::{Nlm4LockArgs, Nlm4LockRes};
use crate::nlm::procedures::unlock::{Nlm4UnlockArgs, Nlm4UnlockRes};
use crate::nlm::{Nlm, Nlm4Stats, OpaqueHandle};
use crate::task::{ProcReply, ProcResult};
use crate::vfs::file::Handle;
use crate::{
    nlm::NlmRes,
    parser::{NlmArgWrapper, NlmArguments},
};

const PENDING_RESOLVER_INITIAL_XID: u32 = 0x8000_0000;

pub struct NlmCommand<B: Buffer> {
    /// Channel used to pass the result to write task.
    pub result_tx: Sender<ProcReply<B>>,
    /// Placeholder for NLM procedure args.
    pub args: NlmArgWrapper,
}

pub struct NlmTask<B, N>
where
    B: Buffer + 'static,
    N: Nlm + Send + Sync + 'static,
{
    /// Shared NLM service implementation.
    nlm_service: Arc<N>,

    /// Channel for commands from client connection tasks
    receiver: Receiver<NlmCommand<B>>,

    /// Resolver state used to mirror successful lock/unlock/cancel operations.
    pending_resolver: PendingResolverTask<B>,
}

impl<B, N> NlmTask<B, N>
where
    B: Buffer + 'static,
    N: Nlm + Send + Sync + 'static,
{
    /// Creates new instance of [`NlmTask`]
    pub fn new(nlm_service: Arc<N>) -> (Self, Sender<NlmCommand<B>>, PendingResolverTask<B>) {
        let (sender, receiver) = async_channel::unbounded::<NlmCommand<B>>();
        let pending_resolver = PendingResolverTask::new();

        let task = Self { nlm_service, receiver, pending_resolver: pending_resolver.clone() };

        (task, sender, pending_resolver)
    }

    /// Spawns the [`NlmTask`] on the current Tokio runtime.
    ///
    /// The task processes NLM commands received from read tasks and
    /// returns results to write tasks.
    ///
    /// # Panics
    ///
    /// If called outside a Tokio runtime context.
    pub fn spawn(self) {
        tokio::spawn(async move { self.run().await });
    }

    /// Main event loop: waits for commands, dispatches to the NLM service,
    /// and sends replies back.
    async fn run(self) {
        let nlm_service = self.nlm_service;
        let receiver = self.receiver;
        let pending_resolver = self.pending_resolver;

        while let Ok(command) = receiver.recv().await {
            let NlmCommand { result_tx, args } = command;
            let NlmArgWrapper { header, proc } = args;
            debug!(xid = header.xid, "nlm task: command received");

            let nlm_result = match *proc {
                NlmArguments::Null => NlmRes::Null,
                NlmArguments::Lock(nlm4_lock_args) => {
                    debug!(xid = header.xid, "nlm task: proc=NLM LOCK");
                    let resolver_args = nlm4_lock_args.clone();
                    let res = nlm_service.lock(nlm4_lock_args).await;
                    let Ok((original_result, granted_locks)) = pending_resolver.apply(
                        header.client_addr,
                        result_tx.clone(),
                        PendingEvent::Lock(resolver_args, res),
                    ) else {
                        return;
                    };
                    send_nlm_replies(header.xid, &result_tx, original_result, granted_locks).await;
                    continue;
                }
                NlmArguments::Unlock(nlm4_unlock_args) => {
                    debug!(xid = header.xid, "nlm task: proc=NLM UNLOCK");
                    let resolver_args = nlm4_unlock_args.clone();
                    let res = nlm_service.unlock(nlm4_unlock_args).await;
                    let Ok((original_result, granted_locks)) = pending_resolver.apply(
                        header.client_addr,
                        result_tx.clone(),
                        PendingEvent::Unlock(resolver_args, res),
                    ) else {
                        return;
                    };
                    send_nlm_replies(header.xid, &result_tx, original_result, granted_locks).await;
                    continue;
                }
                NlmArguments::Test(nlm4_test_args) => {
                    debug!(xid = header.xid, "nlm task: proc=NLM TEST");
                    let res = nlm_service.test(nlm4_test_args).await;
                    NlmRes::Test(Box::new(res))
                }
                NlmArguments::Cancel(nlm4_cancel_args) => {
                    debug!(xid = header.xid, "nlm task: proc=NLM CANCEL");
                    let resolver_args = nlm4_cancel_args.clone();
                    let res = nlm_service.cancel(nlm4_cancel_args).await;
                    let Ok((original_result, granted_locks)) = pending_resolver.apply(
                        header.client_addr,
                        result_tx.clone(),
                        PendingEvent::Cancel(resolver_args, res),
                    ) else {
                        return;
                    };
                    send_nlm_replies(header.xid, &result_tx, original_result, granted_locks).await;
                    continue;
                }
            };

            // TODO:
            // - some logs when occurred error
            // - or retry with fail
            // * but don't stop task
            let _ = result_tx
                .send(ProcReply {
                    xid: header.xid,
                    proc_result: Ok(ProcResult::Nlm4(Box::new(nlm_result))),
                })
                .await;
            debug!(xid = header.xid, "nlm task: reply queued");
        }
    }
}

async fn send_nlm_replies<B: Buffer + 'static>(
    xid: u32,
    result_tx: &Sender<ProcReply<B>>,
    original_result: NlmRes,
    granted_locks: Vec<(u32, Sender<ProcReply<B>>, Nlm4LockRes)>,
) {
    if result_tx
        .send(ProcReply { xid, proc_result: Ok(ProcResult::Nlm4(Box::new(original_result))) })
        .await
        .is_err()
    {
        return;
    }

    for (xid, result_tx, lock_res) in granted_locks {
        if result_tx
            .send(ProcReply {
                xid,
                proc_result: Ok(ProcResult::Nlm4(Box::new(NlmRes::Lock(lock_res)))),
            })
            .await
            .is_err()
        {
            break;
        }
    }
}

enum PendingEvent {
    Lock(Nlm4LockArgs, Nlm4LockRes),
    Unlock(Nlm4UnlockArgs, Nlm4UnlockRes),
    Cancel(Nlm4CancelArgs, Nlm4CancelRes),
}

/// Resolves pending locks outside the NLM service implementation.
pub struct PendingResolverTask<B: Buffer + 'static> {
    inner: Arc<Mutex<PendingResolverState<B>>>,
}

impl<B> Clone for PendingResolverTask<B>
where
    B: Buffer + 'static,
{
    fn clone(&self) -> Self {
        Self { inner: self.inner.clone() }
    }
}

impl<B> PendingResolverTask<B>
where
    B: Buffer + 'static,
{
    fn new() -> Self {
        Self { inner: Arc::new(Mutex::new(PendingResolverState::new())) }
    }

    pub fn register_writer(&self, client_addr: SocketAddr, result_tx: Sender<ProcReply<B>>) {
        let Ok(mut inner) = self.inner.lock() else {
            return;
        };
        inner.register_writer(client_addr, result_tx);
    }

    fn apply(
        &self,
        client_addr: SocketAddr,
        result_tx: Sender<ProcReply<B>>,
        event: PendingEvent,
    ) -> Result<(NlmRes, Vec<(u32, Sender<ProcReply<B>>, Nlm4LockRes)>), ()> {
        self.inner.lock().map_err(|_| ())?.apply(client_addr, result_tx, event)
    }
}

struct PendingResolverState<B: Buffer + 'static> {
    next_xid: AtomicU32,
    writers: HashMap<SocketAddr, Sender<ProcReply<B>>>,
    locks: LockTable,
}

impl<B> PendingResolverState<B>
where
    B: Buffer + 'static,
{
    fn new() -> Self {
        Self {
            next_xid: AtomicU32::new(PENDING_RESOLVER_INITIAL_XID),
            writers: HashMap::new(),
            locks: LockTable::new(),
        }
    }

    fn register_writer(&mut self, client_addr: SocketAddr, result_tx: Sender<ProcReply<B>>) {
        self.writers.insert(client_addr, result_tx);
    }

    fn apply(
        &mut self,
        client_addr: SocketAddr,
        result_tx: Sender<ProcReply<B>>,
        event: PendingEvent,
    ) -> Result<(NlmRes, Vec<(u32, Sender<ProcReply<B>>, Nlm4LockRes)>), ()> {
        self.register_writer(client_addr, result_tx);

        match event {
            PendingEvent::Lock(args, res) => {
                match res.stat {
                    Nlm4Stats::Granted => {
                        self.locks.try_push_granted(
                            args.lock.file_handle.clone(),
                            ActiveLock::from_lock_args(&args),
                            args.reclaim,
                        )?;
                    }
                    Nlm4Stats::Blocked => {
                        self.locks
                            .pending
                            .entry(args.lock.file_handle.clone())
                            .or_default()
                            .push(PendingLock::from_lock_args(&args, client_addr));
                    }
                    _ => {}
                }
                Ok((NlmRes::Lock(res), Vec::new()))
            }
            PendingEvent::Unlock(args, res) => {
                let mut granted = Vec::new();
                if res.stat == Nlm4Stats::Granted {
                    self.locks.remove_by_owner(&args.lock);
                    for (owner_addr, lock_res) in self.locks.grant_pending(&args.lock.file_handle) {
                        let Some(result_tx) = self.writers.get(&owner_addr).cloned() else {
                            continue;
                        };
                        let xid = self.next_xid.fetch_add(1, Ordering::Relaxed);
                        granted.push((xid, result_tx, lock_res));
                    }
                }
                Ok((NlmRes::Unlock(res), granted))
            }
            PendingEvent::Cancel(args, res) => {
                if res.stat == Nlm4Stats::Granted {
                    let target = PendingLock::from_cancel_args(&args);
                    self.locks.apply_granted_cancel(&args.lock.file_handle, &target)?;
                }
                Ok((NlmRes::Cancel(res), Vec::new()))
            }
        }
    }
}

#[derive(Clone)]
struct ActiveLock {
    caller_name: String,
    system_identifier: i32,
    exclusive: bool,
    offset: u64,
    length: u64,
    opaque_handle: OpaqueHandle,
}

impl ActiveLock {
    fn from_lock_args(args: &Nlm4LockArgs) -> Self {
        Self {
            caller_name: args.lock.caller_name.clone(),
            system_identifier: args.lock.system_identifier,
            exclusive: args.exclusive,
            offset: args.lock.lock_offset,
            length: args.lock.lock_length,
            opaque_handle: args.lock.opaque_handle.clone(),
        }
    }

    fn from_pending(pending: &PendingLock) -> Self {
        Self {
            caller_name: pending.caller_name.clone(),
            system_identifier: pending.system_identifier,
            exclusive: pending.exclusive,
            offset: pending.offset,
            length: pending.length,
            opaque_handle: pending.opaque_handle.clone(),
        }
    }
}

#[derive(Clone)]
struct PendingLock {
    caller_name: String,
    system_identifier: i32,
    exclusive: bool,
    offset: u64,
    length: u64,
    opaque_handle: OpaqueHandle,
    cookie: Cookie,
    client_addr: SocketAddr,
}

impl PendingLock {
    fn from_lock_args(args: &Nlm4LockArgs, client_addr: SocketAddr) -> Self {
        Self {
            caller_name: args.lock.caller_name.clone(),
            system_identifier: args.lock.system_identifier,
            exclusive: args.exclusive,
            offset: args.lock.lock_offset,
            length: args.lock.lock_length,
            opaque_handle: args.lock.opaque_handle.clone(),
            cookie: args.cookie,
            client_addr,
        }
    }

    fn from_cancel_args(args: &Nlm4CancelArgs) -> Self {
        Self {
            caller_name: args.lock.caller_name.clone(),
            system_identifier: args.lock.system_identifier,
            exclusive: args.exclusive,
            offset: args.lock.lock_offset,
            length: args.lock.lock_length,
            opaque_handle: args.lock.opaque_handle.clone(),
            cookie: args.cookie,
            client_addr: SocketAddr::from(([0, 0, 0, 0], 0)),
        }
    }
}

impl PartialEq for PendingLock {
    fn eq(&self, other: &Self) -> bool {
        self.caller_name == other.caller_name
            && self.system_identifier == other.system_identifier
            && self.exclusive == other.exclusive
            && self.offset == other.offset
            && self.length == other.length
            && self.opaque_handle == other.opaque_handle
    }
}

struct LockTable {
    by_file: HashMap<Handle, Vec<ActiveLock>>,
    pending: HashMap<Handle, Vec<PendingLock>>,
}

impl LockTable {
    fn new() -> Self {
        Self { by_file: HashMap::new(), pending: HashMap::new() }
    }

    fn try_push_granted(
        &mut self,
        file_handle: Handle,
        new_lock: ActiveLock,
        reclaim: bool,
    ) -> Result<(), ()> {
        if !reclaim && self.find_conflict(&file_handle, &new_lock) {
            return Err(());
        }
        self.push_or_replace(file_handle, new_lock);
        Ok(())
    }

    fn push_or_replace(&mut self, file_handle: Handle, new_lock: ActiveLock) {
        let locks = self.by_file.entry(file_handle).or_default();
        drain_overlapping(
            locks,
            &new_lock.caller_name,
            new_lock.system_identifier,
            new_lock.offset,
            new_lock.length,
        );
        locks.push(new_lock);
        merge_adjacent(locks);
    }

    fn remove_by_owner(&mut self, lock: &Nlm4Lock) {
        let active_locks = match self.by_file.get_mut(&lock.file_handle) {
            Some(locks) => locks,
            None => return,
        };

        drain_overlapping(
            active_locks,
            &lock.caller_name,
            lock.system_identifier,
            lock.lock_offset,
            lock.lock_length,
        );

        if active_locks.is_empty() {
            self.by_file.remove(&lock.file_handle);
        }
    }

    fn apply_granted_cancel(
        &mut self,
        file_handle: &Handle,
        target: &PendingLock,
    ) -> Result<(), ()> {
        if self.remove_pending(file_handle, target) || self.has_active_lock(file_handle, target) {
            return Ok(());
        }

        Err(())
    }

    fn remove_pending(&mut self, file_handle: &Handle, target: &PendingLock) -> bool {
        let pending_requests = match self.pending.get_mut(file_handle) {
            Some(requests) => requests,
            None => return false,
        };

        let before = pending_requests.len();
        pending_requests.retain(|request| request != target);
        let removed = pending_requests.len() < before;

        if pending_requests.is_empty() {
            self.pending.remove(file_handle);
        }

        removed
    }

    fn has_active_lock(&self, file_handle: &Handle, target: &PendingLock) -> bool {
        self.by_file.get(file_handle).is_some_and(|locks| {
            locks.iter().any(|lock| {
                lock.caller_name == target.caller_name
                    && lock.system_identifier == target.system_identifier
                    && lock.exclusive == target.exclusive
                    && ranges_overlap(lock.offset, lock.length, target.offset, target.length)
            })
        })
    }

    fn find_conflict(&self, file_handle: &Handle, request: &ActiveLock) -> bool {
        self.by_file.get(file_handle).is_some_and(|locks| {
            locks.iter().any(|lock| {
                let is_same_owner = lock.caller_name == request.caller_name
                    && lock.system_identifier == request.system_identifier
                    && lock.opaque_handle == request.opaque_handle;

                !is_same_owner
                    && (request.exclusive || lock.exclusive)
                    && ranges_overlap(lock.offset, lock.length, request.offset, request.length)
            })
        })
    }

    fn grant_pending(&mut self, file_handle: &Handle) -> Vec<(SocketAddr, Nlm4LockRes)> {
        let pending_requests = self.pending.remove(file_handle).unwrap_or_default();
        let mut granted = Vec::new();
        let mut still_pending = Vec::new();

        for request in pending_requests {
            let request_as_active = ActiveLock::from_pending(&request);
            if self.find_conflict(file_handle, &request_as_active) {
                still_pending.push(request);
            } else {
                self.push_or_replace(file_handle.clone(), request_as_active);
                granted.push((
                    request.client_addr,
                    Nlm4LockRes { cookie: request.cookie, stat: Nlm4Stats::Granted },
                ));
            }
        }

        if !still_pending.is_empty() {
            self.pending.insert(file_handle.clone(), still_pending);
        }

        granted
    }
}

const LEN_REMAINING: u64 = 0;

fn ranges_overlap(start1: u64, len1: u64, start2: u64, len2: u64) -> bool {
    let end1 = calculate_end_of_interval(start1, len1);
    let end2 = calculate_end_of_interval(start2, len2);
    start1 <= end2 && start2 <= end1
}

fn calculate_end_of_interval(start: u64, len: u64) -> u64 {
    match len {
        LEN_REMAINING => u64::MAX,
        _ => start.saturating_add(len).saturating_sub(1),
    }
}

fn split_lock(lock: ActiveLock, unlock_start: u64, unlock_len: u64) -> Vec<ActiveLock> {
    let lock_start = lock.offset;
    let lock_len = lock.length;
    let lock_end = calculate_end_of_interval(lock_start, lock_len);
    let unlock_end = calculate_end_of_interval(unlock_start, unlock_len);

    let mut fragments = Vec::new();

    if lock_start < unlock_start {
        fragments.push(ActiveLock {
            caller_name: lock.caller_name.clone(),
            system_identifier: lock.system_identifier,
            exclusive: lock.exclusive,
            offset: lock_start,
            length: unlock_start - lock_start,
            opaque_handle: lock.opaque_handle.clone(),
        });
    }

    if unlock_end < lock_end {
        let right_len = if lock_len == 0 { 0 } else { lock_end - unlock_end };
        fragments.push(ActiveLock {
            caller_name: lock.caller_name,
            system_identifier: lock.system_identifier,
            exclusive: lock.exclusive,
            offset: unlock_end + 1,
            length: right_len,
            opaque_handle: lock.opaque_handle,
        });
    }

    fragments
}

fn drain_overlapping(
    locks: &mut Vec<ActiveLock>,
    caller_name: &str,
    system_identifier: i32,
    start: u64,
    len: u64,
) {
    let mut i = 0;
    while i < locks.len() {
        if locks[i].caller_name != caller_name || locks[i].system_identifier != system_identifier {
            i += 1;
            continue;
        }
        if !ranges_overlap(locks[i].offset, locks[i].length, start, len) {
            i += 1;
            continue;
        }
        let old = locks.swap_remove(i);
        locks.extend(split_lock(old, start, len));
    }
}

fn merge_adjacent(locks: &mut Vec<ActiveLock>) {
    locks.sort_by(|a, b| {
        a.caller_name
            .cmp(&b.caller_name)
            .then(a.system_identifier.cmp(&b.system_identifier))
            .then(a.exclusive.cmp(&b.exclusive))
            .then(a.offset.cmp(&b.offset))
    });

    let mut write = 0;
    for read in 1..locks.len() {
        if locks[write].caller_name == locks[read].caller_name
            && locks[write].system_identifier == locks[read].system_identifier
            && locks[write].exclusive == locks[read].exclusive
        {
            let write_end = calculate_end_of_interval(locks[write].offset, locks[write].length);
            let read_end = calculate_end_of_interval(locks[read].offset, locks[read].length);

            let adjacent_or_overlapping =
                if write_end == u64::MAX { true } else { write_end + 1 >= locks[read].offset };

            if adjacent_or_overlapping {
                if locks[write].length == 0 || locks[read].length == 0 {
                    locks[write].length = 0;
                } else {
                    let new_end = std::cmp::max(write_end, read_end);
                    locks[write].length = new_end - locks[write].offset + 1;
                }
                continue;
            }
        }
        write += 1;
        if write != read {
            locks.swap(write, read);
        }
    }
    locks.truncate(write + 1);
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use crate::consts::{nfsv3::NFS3_FHSIZE, nlm::OPAQUE_HANDLE_SIZE};
    use crate::nlm::cookie::Cookie;
    use crate::nlm::lock::Nlm4Lock;
    use crate::nlm::procedures::cancel::Nlm4CancelArgs;
    use crate::nlm::procedures::lock::Nlm4LockArgs;
    use crate::nlm::procedures::unlock::Nlm4UnlockArgs;
    use crate::nlm::OpaqueHandle;
    use crate::parser::RpcHeader;
    use crate::rpc::Credential;
    use crate::service::nlm::NlmService;
    use crate::vfs::file::Handle;

    use super::*;

    fn alice_addr() -> SocketAddr {
        SocketAddr::from(([127, 0, 0, 1], 10001))
    }

    fn bob_addr() -> SocketAddr {
        SocketAddr::from(([127, 0, 0, 1], 10002))
    }

    struct TestBuffer;

    impl Buffer for TestBuffer {
        fn chunks(&self) -> impl Iterator<Item = &[u8]> + Send + '_ {
            std::iter::empty()
        }

        fn chunks_mut(&mut self) -> impl Iterator<Item = &mut [u8]> + Send + '_ {
            std::iter::empty()
        }

        fn len(&self) -> usize {
            0
        }

        fn is_empty(&self) -> bool {
            true
        }

        fn empty() -> Self
        where
            Self: Sized,
        {
            TestBuffer
        }
    }

    #[tokio::test]
    async fn successful_unlock_resolves_pending_locks() {
        let (task, command_tx, pending_resolver) =
            NlmTask::<TestBuffer, _>::new(Arc::new(NlmService::new()));
        task.spawn();

        let (alice_result_tx, alice_result_rx) = async_channel::unbounded();
        let (bob_result_tx, bob_result_rx) = async_channel::unbounded();
        pending_resolver.register_writer(alice_addr(), alice_result_tx.clone());
        pending_resolver.register_writer(bob_addr(), bob_result_tx.clone());

        command_tx
            .send(command(
                1,
                alice_addr(),
                alice_result_tx.clone(),
                NlmArguments::Lock(lock_args(1, "alice", 100, false)),
            ))
            .await
            .unwrap();
        assert_lock_reply(&alice_result_rx, 1, Nlm4Stats::Granted).await;

        command_tx
            .send(command(
                2,
                bob_addr(),
                bob_result_tx,
                NlmArguments::Lock(lock_args(2, "bob", 200, true)),
            ))
            .await
            .unwrap();
        assert_lock_reply(&bob_result_rx, 2, Nlm4Stats::Blocked).await;

        command_tx
            .send(command(
                3,
                alice_addr(),
                alice_result_tx,
                NlmArguments::Unlock(unlock_args(3, "alice", 100)),
            ))
            .await
            .unwrap();
        assert_unlock_reply(&alice_result_rx, 3, Nlm4Stats::Granted).await;
        assert_lock_reply(&bob_result_rx, PENDING_RESOLVER_INITIAL_XID, Nlm4Stats::Granted).await;
        assert_no_reply(&alice_result_rx).await;
    }

    #[tokio::test]
    async fn successful_cancel_removes_pending_lock_from_resolver() {
        let (task, command_tx, pending_resolver) =
            NlmTask::<TestBuffer, _>::new(Arc::new(NlmService::new()));
        task.spawn();

        let (result_tx, result_rx) = async_channel::unbounded();
        pending_resolver.register_writer(alice_addr(), result_tx.clone());

        command_tx
            .send(command(
                1,
                alice_addr(),
                result_tx.clone(),
                NlmArguments::Lock(lock_args(1, "alice", 100, false)),
            ))
            .await
            .unwrap();
        assert_lock_reply(&result_rx, 1, Nlm4Stats::Granted).await;

        command_tx
            .send(command(
                2,
                alice_addr(),
                result_tx.clone(),
                NlmArguments::Lock(lock_args(2, "bob", 200, true)),
            ))
            .await
            .unwrap();
        assert_lock_reply(&result_rx, 2, Nlm4Stats::Blocked).await;

        command_tx
            .send(command(
                3,
                alice_addr(),
                result_tx.clone(),
                NlmArguments::Cancel(cancel_args(3, "bob", 200)),
            ))
            .await
            .unwrap();
        assert_cancel_reply(&result_rx, 3, Nlm4Stats::Granted).await;

        command_tx
            .send(command(
                4,
                alice_addr(),
                result_tx,
                NlmArguments::Unlock(unlock_args(4, "alice", 100)),
            ))
            .await
            .unwrap();
        assert_unlock_reply(&result_rx, 4, Nlm4Stats::Granted).await;
        assert_no_reply(&result_rx).await;
    }

    #[tokio::test]
    async fn pending_resolver_stops_without_reply_when_apply_fails() {
        let (result_tx, result_rx) = async_channel::unbounded();
        let pending_resolver = PendingResolverTask::new();

        let (original_result, granted_locks) = pending_resolver
            .apply(
                alice_addr(),
                result_tx.clone(),
                PendingEvent::Lock(
                    lock_args(1, "alice", 100, false),
                    Nlm4LockRes { cookie: Cookie::new(1), stat: Nlm4Stats::Granted },
                ),
            )
            .unwrap();
        send_nlm_replies(1, &result_tx, original_result, granted_locks).await;
        assert_lock_reply(&result_rx, 1, Nlm4Stats::Granted).await;

        assert!(pending_resolver
            .apply(
                bob_addr(),
                result_tx.clone(),
                PendingEvent::Lock(
                    lock_args(2, "bob", 200, false),
                    Nlm4LockRes { cookie: Cookie::new(2), stat: Nlm4Stats::Granted },
                ),
            )
            .is_err());
        assert_no_reply(&result_rx).await;
    }

    fn command(
        xid: u32,
        client_addr: SocketAddr,
        result_tx: Sender<ProcReply<TestBuffer>>,
        args: NlmArguments,
    ) -> NlmCommand<TestBuffer> {
        NlmCommand {
            result_tx,
            args: NlmArgWrapper {
                header: RpcHeader { xid, client_addr, cred: Credential::None },
                proc: Box::new(args),
            },
        }
    }

    async fn assert_lock_reply(
        result_rx: &Receiver<ProcReply<TestBuffer>>,
        xid: u32,
        stat: Nlm4Stats,
    ) {
        let reply = recv_reply(result_rx).await;
        assert_eq!(reply.xid, xid);
        match reply.proc_result.unwrap() {
            ProcResult::Nlm4(result) => match *result {
                NlmRes::Lock(result) => assert_eq!(result.stat, stat),
                _ => panic!("expected NLM LOCK reply"),
            },
            _ => panic!("expected NLM reply"),
        }
    }

    async fn assert_unlock_reply(
        result_rx: &Receiver<ProcReply<TestBuffer>>,
        xid: u32,
        stat: Nlm4Stats,
    ) {
        let reply = recv_reply(result_rx).await;
        assert_eq!(reply.xid, xid);
        match reply.proc_result.unwrap() {
            ProcResult::Nlm4(result) => match *result {
                NlmRes::Unlock(result) => assert_eq!(result.stat, stat),
                _ => panic!("expected NLM UNLOCK reply"),
            },
            _ => panic!("expected NLM reply"),
        }
    }

    async fn assert_cancel_reply(
        result_rx: &Receiver<ProcReply<TestBuffer>>,
        xid: u32,
        stat: Nlm4Stats,
    ) {
        let reply = recv_reply(result_rx).await;
        assert_eq!(reply.xid, xid);
        match reply.proc_result.unwrap() {
            ProcResult::Nlm4(result) => match *result {
                NlmRes::Cancel(result) => assert_eq!(result.stat, stat),
                _ => panic!("expected NLM CANCEL reply"),
            },
            _ => panic!("expected NLM reply"),
        }
    }

    async fn assert_no_reply(result_rx: &Receiver<ProcReply<TestBuffer>>) {
        assert!(tokio::time::timeout(Duration::from_millis(100), result_rx.recv()).await.is_err());
    }

    async fn recv_reply(result_rx: &Receiver<ProcReply<TestBuffer>>) -> ProcReply<TestBuffer> {
        tokio::time::timeout(Duration::from_secs(1), result_rx.recv())
            .await
            .expect("timed out waiting for NLM reply")
            .expect("result channel closed")
    }

    fn lock_args(
        cookie: u64,
        caller_name: &str,
        system_identifier: i32,
        block: bool,
    ) -> Nlm4LockArgs {
        Nlm4LockArgs {
            cookie: Cookie::new(cookie),
            block,
            exclusive: true,
            lock: nlm_lock(caller_name, system_identifier),
            reclaim: false,
            state: 0,
        }
    }

    fn unlock_args(cookie: u64, caller_name: &str, system_identifier: i32) -> Nlm4UnlockArgs {
        Nlm4UnlockArgs {
            cookie: Cookie::new(cookie),
            lock: nlm_lock(caller_name, system_identifier),
        }
    }

    fn cancel_args(cookie: u64, caller_name: &str, system_identifier: i32) -> Nlm4CancelArgs {
        Nlm4CancelArgs {
            cookie: Cookie::new(cookie),
            block: true,
            exclusive: true,
            lock: nlm_lock(caller_name, system_identifier),
        }
    }

    fn nlm_lock(caller_name: &str, system_identifier: i32) -> Nlm4Lock {
        Nlm4Lock {
            caller_name: caller_name.into(),
            file_handle: Handle([1; NFS3_FHSIZE]),
            opaque_handle: OpaqueHandle::new(vec![1; OPAQUE_HANDLE_SIZE]).unwrap(),
            system_identifier,
            lock_offset: 0,
            lock_length: 100,
        }
    }
}
