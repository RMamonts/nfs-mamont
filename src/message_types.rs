#![allow(dead_code)]
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::SendError;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

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
    sender: UnboundedSender<EarlyReply>,
}

impl EarlyReplySender {
    /// Sends a null early reply for the given transaction ID
    pub async fn send_null(&self, xid: u32) -> Result<(), SendError<EarlyReply>> {
        self.sender.send(EarlyReply { xid, result: EarlyResult::Null })
    }

    /// Sends an RPC error early reply for the given transaction ID
    pub async fn send_error(&self, xid: u32) -> Result<(), SendError<EarlyReply>> {
        self.sender.send(EarlyReply { xid, result: EarlyResult::RPCError })
    }
}

/// Receiver for early reply messages
pub struct EarlyReplyRecv {
    recv: UnboundedReceiver<EarlyReply>,
}

impl EarlyReplyRecv {
    /// Receives the next early reply from the channel
    pub async fn recv(&mut self) -> Option<EarlyReply> {
        self.recv.recv().await
    }
}

/// Receiver for procedure reply messages
pub struct ReplyRecv {
    recv: UnboundedReceiver<Reply>,
}

impl ReplyRecv {
    /// Receives the next procedure reply from the channel
    pub async fn recv(&mut self) -> Option<Reply> {
        self.recv.recv().await
    }
}

/// Sender for procedure reply messages
pub struct ReplySender {
    sender: UnboundedSender<Reply>,
}

impl ReplySender {
    /// Sends a successful procedure reply
    pub async fn send_ok_reply(&self, xid: u32, res: ProcResOk) -> Result<(), SendError<Reply>> {
        self.sender.send(Reply { xid, result: ProcResult::Ok(res) })
    }

    /// Sends an error procedure reply
    pub async fn send_error_reply(&self, xid: u32, err: ProcError) -> Result<(), SendError<Reply>> {
        self.sender.send(Reply { xid, result: ProcResult::Error(err) })
    }
}

/// Sender for procedure call messages
pub struct ProcSender {
    sender: UnboundedSender<Procedure>,
}

impl ProcSender {
    /// Sends an NFSv3 procedure call with the given transaction ID
    pub async fn send_nfsv3(&self, xid: u32) -> Result<(), SendError<Procedure>> {
        self.sender.send(Procedure { xid, args: Command::NFSv3Command })
    }

    /// Sends a mount procedure call with the given transaction ID
    pub async fn send_mount(&self, xid: u32) -> Result<(), SendError<Procedure>> {
        self.sender.send(Procedure { xid, args: Command::MountCommand })
    }
}

/// Receiver for procedure call messages
pub struct ProcRecv {
    recv: UnboundedReceiver<Procedure>,
}

impl ProcRecv {
    /// Receives the next procedure call from the channel
    pub async fn recv(&mut self) -> Option<Procedure> {
        self.recv.recv().await
    }
}

/// Creates a new unbounded channel for procedure calls
pub fn create_proc_channel() -> (ProcSender, ProcRecv) {
    let (sender, recv) = mpsc::unbounded_channel::<Procedure>();
    (ProcSender { sender }, ProcRecv { recv })
}

/// Creates a new unbounded channel for procedure replies
pub fn create_reply_channel() -> (ReplySender, ReplyRecv) {
    let (sender, recv) = mpsc::unbounded_channel::<Reply>();
    (ReplySender { sender }, ReplyRecv { recv })
}

/// Creates a new unbounded channel for early replies
pub fn create_early_reply_channel() -> (EarlyReplySender, EarlyReplyRecv) {
    let (sender, recv) = mpsc::unbounded_channel::<EarlyReply>();
    (EarlyReplySender { sender }, EarlyReplyRecv { recv })
}
