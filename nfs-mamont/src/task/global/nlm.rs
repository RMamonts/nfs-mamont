//! NLMv4 task dispatcher.
//!
//! Runs a background task that receives parsed NLM procedure calls from
//! connection read tasks, forwards them to the [`Nlm`] service, and sends
//! the serialized reply back to the appropriate write task.

use async_channel::{Receiver, Sender};
use futures_util::stream::FuturesUnordered;
use futures_util::StreamExt;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tracing::debug;

use crate::allocator::Buffer;
use crate::nlm::procedures::lock::{Nlm4LockArgs, Nlm4LockRes};
use crate::nlm::{Nlm, Nlm4Stats};
use crate::task::{ProcReply, ProcResult};
use crate::{
    nlm::NlmRes,
    parser::{NlmArgWrapper, NlmArguments},
};

/// How long a blocked LOCK future waits between attempts to grant the lock.
const LOCK_RETRY_INTERVAL: Duration = Duration::from_millis(100);

/// A bounded future that retries a LOCK request until it is no longer blocked.
///
/// Each future continuously re-submits the request to the [`Nlm`] service and
/// sleeps for [`LOCK_RETRY_INTERVAL`] between attempts. When the service stops
/// reporting [`Nlm4Stats::Blocked`], the future yields the reply to send back
/// to the client that issued the original call, together with that client's
/// address so the NLM task can route it to the right write task.
type BlockedGrant<B> = (SocketAddr, ProcReply<B>);
type GrantFuture<B> =
    Pin<Box<dyn futures_util::future::Future<Output = BlockedGrant<B>> + Send + 'static>>;

/// Registration message from connection tasks, notifying the NLM task that a
/// new client connection is active and providing the channel through which
/// replies for that client must be sent to its write task.
pub struct NlmRegistration<B: Buffer> {
    /// Address of the newly registered client connection.
    pub client_addr: SocketAddr,
    /// Sender end of this client's write task channel, to route replies to.
    pub result_tx: Sender<ProcReply<B>>,
}

/// Command carrying parsed NLM procedure arguments from a connection read task.
pub struct NlmCommand {
    /// Placeholder for NLM procedure args. The client address is read from the
    /// RPC header to determine which write task to deliver the reply to.
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
    receiver: Receiver<NlmCommand>,

    /// Channel for client registration requests from connection tasks
    registration_receiver: Receiver<NlmRegistration<B>>,

    /// Maps an active client address to the channel used to send replies
    /// back to that client's write task.
    client_map: HashMap<SocketAddr, Sender<ProcReply<B>>>,

    /// Set of futures that are busy trying to grant a previously blocked LOCK
    /// request. They complete once the lock can be granted or fails with an
    /// error, at which point the reply is routed back to the client.
    blocked_grants: FuturesUnordered<GrantFuture<B>>,
}

impl<B, N> NlmTask<B, N>
where
    B: Buffer + 'static,
    N: Nlm + Send + Sync + 'static,
{
    /// Creates new instance of [`NlmTask`]
    pub fn new(nlm_service: Arc<N>) -> (Self, Sender<NlmCommand>, Sender<NlmRegistration<B>>) {
        let (sender, receiver) = async_channel::unbounded::<NlmCommand>();
        let (registration_sender, registration_receiver) =
            async_channel::unbounded::<NlmRegistration<B>>();

        let task = Self {
            nlm_service,
            receiver,
            registration_receiver,
            client_map: HashMap::new(),
            blocked_grants: FuturesUnordered::new(),
        };

        (task, sender, registration_sender)
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

    /// Main event loop: waits for commands, client registrations and completed
    /// grant futures, dispatches to the NLM service, and sends replies back to
    /// the write task of the originating client.
    async fn run(mut self) {
        loop {
            tokio::select! {
                // TODO: more careful error handling
                reg = self.registration_receiver.recv() => {
                    match reg {
                        Ok(reg) => {
                            self.client_map.insert(reg.client_addr, reg.result_tx);
                            debug!(client = %reg.client_addr, "nlm task: client registered");
                        }
                        Err(_) => break,
                    }
                }
                command = self.receiver.recv() => {
                    let Ok(command) = command else { break };
                    let NlmCommand { args } = command;
                    self.handle_command(args).await;
                }
                Some((client_addr, reply)) = self.blocked_grants.next() => {
                    // TODO: add waiting of reply after sending resolved pending lock
                    self.send_reply(client_addr, reply).await;
                }
            }
        }
    }

    async fn handle_command(&mut self, args: NlmArgWrapper) {
        let nlm_service = Arc::clone(&self.nlm_service);
        let NlmArgWrapper { header, proc } = args;
        debug!(xid = header.xid, "nlm task: command received");

        match *proc {
            NlmArguments::Lock(lock_args) => {
                debug!(xid = header.xid, "nlm task: proc=NLM LOCK");
                let res = nlm_service.lock(lock_args.clone()).await;
                if res.stat == Nlm4Stats::Blocked {
                    debug!(xid = header.xid, "nlm task: LOCK blocked, scheduling retry");
                    self.schedule_lock_grant(
                        nlm_service,
                        lock_args,
                        header.xid,
                        header.client_addr,
                    );
                }
                self.send_reply(
                    header.client_addr,
                    ProcReply {
                        xid: header.xid,
                        proc_result: Ok(ProcResult::Nlm4(Box::new(NlmRes::Lock(res)))),
                    },
                )
                .await;
            }
            NlmArguments::Unlock(nlm4_unlock_args) => {
                debug!(xid = header.xid, "nlm task: proc=NLM UNLOCK");
                let res = nlm_service.unlock(nlm4_unlock_args).await;
                self.send_reply(
                    header.client_addr,
                    ProcReply {
                        xid: header.xid,
                        proc_result: Ok(ProcResult::Nlm4(Box::new(NlmRes::Unlock(res)))),
                    },
                )
                .await;
            }
            NlmArguments::Test(nlm4_test_args) => {
                debug!(xid = header.xid, "nlm task: proc=NLM TEST");
                let res = nlm_service.test(nlm4_test_args).await;
                self.send_reply(
                    header.client_addr,
                    ProcReply {
                        xid: header.xid,
                        proc_result: Ok(ProcResult::Nlm4(Box::new(NlmRes::Test(Box::new(res))))),
                    },
                )
                .await;
            }
            NlmArguments::Cancel(nlm4_cancel_args) => {
                debug!(xid = header.xid, "nlm task: proc=NLM CANCEL");
                let res = nlm_service.cancel(nlm4_cancel_args).await;
                self.send_reply(
                    header.client_addr,
                    ProcReply {
                        xid: header.xid,
                        proc_result: Ok(ProcResult::Nlm4(Box::new(NlmRes::Cancel(res)))),
                    },
                )
                .await;
            }
            NlmArguments::Null => {
                self.send_reply(
                    header.client_addr,
                    ProcReply {
                        xid: header.xid,
                        proc_result: Ok(ProcResult::Nlm4(Box::new(NlmRes::Null))),
                    },
                )
                .await;
            }
        }
    }

    /// Registers a future that keeps retrying a blocked LOCK request until the
    /// service stops reporting it as [`Nlm4Stats::Blocked`].
    fn schedule_lock_grant(
        &mut self,
        nlm_service: Arc<N>,
        lock_args: Nlm4LockArgs,
        xid: u32,
        client_addr: SocketAddr,
    ) {
        let future = Box::pin(async move {
            let res = lock_until_granted(nlm_service, lock_args).await;
            (
                client_addr,
                ProcReply { xid, proc_result: Ok(ProcResult::Nlm4(Box::new(NlmRes::Lock(res)))) },
            )
        });
        self.blocked_grants.push(future);
    }

    /// Routes a reply to the write task of the given client.
    async fn send_reply(&mut self, client_addr: SocketAddr, reply: ProcReply<B>) {
        let Some(result_tx) = self.client_map.get(&client_addr) else {
            debug!(client = %client_addr, xid = reply.xid, "nlm task: no registered client, dropping reply");
            return;
        };

        // TODO:
        // - some logs when occurred error
        // - or retry with fail
        // * but don't stop task
        // TODO: need to modify mapping in case WriteTask is dead
        let xid = reply.xid;
        let _ = result_tx.send(reply).await;
        debug!(xid, "nlm task: reply queued");
    }
}

/// Re-submits a LOCK request to the service until it is no longer blocked.
///
/// While the service reports [`Nlm4Stats::Blocked`], the loop sleeps for
/// [`LOCK_RETRY_INTERVAL`] before trying again. Any non-blocking result
/// (granted or error status) is returned as the final outcome.
async fn lock_until_granted<N>(nlm_service: Arc<N>, args: Nlm4LockArgs) -> Nlm4LockRes
where
    N: Nlm,
{
    loop {
        let res = nlm_service.lock(args.clone()).await;
        if res.stat == Nlm4Stats::Blocked {
            tokio::time::sleep(LOCK_RETRY_INTERVAL).await;
            continue;
        }
        return res;
    }
}
