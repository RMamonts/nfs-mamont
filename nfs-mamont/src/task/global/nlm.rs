//! NLMv4 task dispatcher.
//!
//! Runs a background task that receives parsed NLM procedure calls from
//! connection read tasks, forwards them to the [`Nlm`] service, and sends
//! the serialized reply back to the appropriate write task.

use async_channel::{Receiver, Sender};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::debug;

use crate::allocator::Buffer;
use crate::nlm::Nlm;
use crate::task::{ProcReply, ProcResult};
use crate::{
    nlm::NlmRes,
    parser::{NlmArgWrapper, NlmArguments},
};

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
}

impl<B, N> NlmTask<B, N>
where
    B: Buffer + 'static,
    N: Nlm + Send + Sync + 'static,
{
    /// Creates new instance of [`NlmTask`]
    pub fn new(
        nlm_service: Arc<N>,
    ) -> (
        Self,
        Sender<NlmCommand>,
        Sender<NlmRegistration<B>>,
    ) {
        let (sender, receiver) = async_channel::unbounded::<NlmCommand>();
        let (registration_sender, registration_receiver) =
            async_channel::unbounded::<NlmRegistration<B>>();

        let task = Self {
            nlm_service,
            receiver,
            registration_receiver,
            client_map: HashMap::new(),
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

    /// Main event loop: waits for commands and client registrations,
    /// dispatches to the NLM service, and sends replies back to the
    /// write task of the originating client.
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
            }
        }
    }

    async fn handle_command(&mut self, args: NlmArgWrapper) {
        let nlm_service = &*self.nlm_service;
        let NlmArgWrapper { header, proc } = args;
        debug!(xid = header.xid, "nlm task: command received");

        let nlm_result = match *proc {
            NlmArguments::Null => NlmRes::Null,
            NlmArguments::Lock(nlm4_lock_args) => {
                debug!(xid = header.xid, "nlm task: proc=NLM LOCK");
                let res = nlm_service.lock(nlm4_lock_args).await;
                NlmRes::Lock(res)
            }
            NlmArguments::Unlock(nlm4_unlock_args) => {
                debug!(xid = header.xid, "nlm task: proc=NLM UNLOCK");
                let res = nlm_service.unlock(nlm4_unlock_args).await;
                NlmRes::Unlock(res)
            }
            NlmArguments::Test(nlm4_test_args) => {
                debug!(xid = header.xid, "nlm task: proc=NLM TEST");
                let res = nlm_service.test(nlm4_test_args).await;
                NlmRes::Test(Box::new(res))
            }
            NlmArguments::Cancel(nlm4_cancel_args) => {
                debug!(xid = header.xid, "nlm task: proc=NLM CANCEL");
                let res = nlm_service.cancel(nlm4_cancel_args).await;
                NlmRes::Cancel(res)
            }
        };

        // Route the reply to the write task of the client that issued the call.
        let result_tx = match self.client_map.get(&header.client_addr) {
            Some(tx) => tx,
            None => {
                debug!(client = %header.client_addr, xid = header.xid, "nlm task: no registered client, dropping reply");
                return;
            }
        };

        // TODO:
        // - some logs when occurred error
        // - or retry with fail
        // * but don't stop task
        // TODO: need to modify mapping in case WriteTask is dead
        let _ = result_tx
            .send(ProcReply {
                xid: header.xid,
                proc_result: Ok(ProcResult::Nlm4(Box::new(nlm_result))),
            })
            .await;
        debug!(xid = header.xid, "nlm task: reply queued");
    }
}
