#![allow(dead_code)]

use tokio::sync::mpsc;
use tokio::sync::mpsc::error::SendError;
use tokio::sync::mpsc::{Receiver, Sender};

/// Represents a procedure call with transaction ID and command arguments
pub struct Procedure {
    xid: u32,
    args: Command,
}

/// Enum of supported protocol commands
pub enum Command {
    NFSv3Command,
    MountCommand,
}

/// Response container with transaction ID and procedure result
pub struct Reply {
    pub xid: u32,
    pub result: ProcResult,
}

/// Result of procedure execution - either success or error
pub enum ProcResult {
    Error(ProcError),
    Ok(ProcResOk),
}

/// Error types for different protocol operations
pub enum ProcError {
    NFSv3Error,
    MountError,
}

/// Success response types for different protocol operations
pub enum ProcResOk {
    NFSv3ResOk,
    MountResok,
}

/// Early reply sent before full procedure completion
pub struct EarlyReply {
    pub xid: u32,
    pub result: EarlyResult,
}

/// Possible outcomes for early reply messages
pub enum EarlyResult {
    RPCError,
    Null,
}

/// Sender for early reply messages
pub struct EarlyReplySender {
    sender: Sender<EarlyReply>,
}

impl EarlyReplySender {
    /// Sends a null early reply for the given transaction ID
    pub async fn send_null(&self, xid: u32) -> Result<(), SendError<EarlyReply>> {
        self.sender.send(EarlyReply { xid, result: EarlyResult::Null }).await
    }

    /// Sends an RPC error early reply for the given transaction ID
    pub async fn send_error(&self, xid: u32) -> Result<(), SendError<EarlyReply>> {
        self.sender.send(EarlyReply { xid, result: EarlyResult::RPCError }).await
    }
}

/// Sender for procedure reply messages
pub struct ReplySender {
    sender: Sender<Reply>,
}

impl ReplySender {
    /// Sends a successful procedure reply
    pub async fn send_ok_reply(&self, xid: u32, res: ProcResOk) -> Result<(), SendError<Reply>> {
        self.sender.send(Reply { xid, result: ProcResult::Ok(res) }).await
    }

    /// Sends an error procedure reply
    pub async fn send_error_reply(&self, xid: u32, err: ProcError) -> Result<(), SendError<Reply>> {
        self.sender.send(Reply { xid, result: ProcResult::Error(err) }).await
    }
}

/// Sender for procedure call messages
pub struct ProcSender {
    sender: Sender<Procedure>,
}

impl ProcSender {
    /// Sends an NFSv3 procedure call with the given transaction ID
    pub async fn send_nfsv3(&self, xid: u32) -> Result<(), SendError<Procedure>> {
        self.sender.send(Procedure { xid, args: Command::NFSv3Command }).await
    }

    /// Sends a mount procedure call with the given transaction ID
    pub async fn send_mount(&self, xid: u32) -> Result<(), SendError<Procedure>> {
        self.sender.send(Procedure { xid, args: Command::MountCommand }).await
    }
}

/// Receiver for procedure call messages
pub struct ProcRecv {
    recv: Receiver<Procedure>,
}

impl ProcRecv {
    /// Receives the next procedure call from the channel
    pub async fn recv(&mut self) -> Option<Procedure> {
        self.recv.recv().await
    }
}

/// Creates a new channel for procedure calls
pub fn create_proc_channel(size: usize) -> (ProcSender, ProcRecv) {
    let (sender, recv) = mpsc::channel::<Procedure>(size);
    (ProcSender { sender }, ProcRecv { recv })
}

/// Creates a new channel for procedure replies
pub fn create_reply_channel(size: usize) -> (ReplySender, Receiver<Reply>) {
    let (sender, recv) = mpsc::channel::<Reply>(size);
    (ReplySender { sender }, recv)
}

/// Creates a new channel for early replies
pub fn create_early_reply_channel(size: usize) -> (EarlyReplySender, Receiver<EarlyReply>) {
    let (sender, recv) = mpsc::channel::<EarlyReply>(size);
    (EarlyReplySender { sender }, recv)
}
