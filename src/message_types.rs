#![allow(dead_code)]
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::SendError;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

pub struct Procedure {
    xid: u32,
    args: Command,
}

// variants should be parametrized with arg-structures by rfc
pub enum Command {
    NFSv3Command,
    MountCommand,
}

pub struct Reply {
    pub xid: u32,
    pub result: ProcResult,
}

pub enum ProcResult {
    Error(ProcError),
    Ok(ProcResOk),
}

// variants should be parametrized with arg-structures by rfc
pub enum ProcError {
    NFSv3Error,
    MountError,
}
pub enum ProcResOk {
    NFSv3ResOk,
    MountResok,
}

pub struct EarlyReply {
    pub xid: u32,
    pub result: EarlyResult,
}

pub enum EarlyResult {
    RPCError,
    Null,
}

pub struct EarlyReplySender {
    sender: UnboundedSender<EarlyReply>,
}

impl EarlyReplySender {
    pub async fn send_null(&self, xid: u32) -> Result<(), SendError<EarlyReply>> {
        self.sender.send(EarlyReply { xid, result: EarlyResult::Null })
    }

    pub async fn send_error(&self, xid: u32) -> Result<(), SendError<EarlyReply>> {
        self.sender.send(EarlyReply { xid, result: EarlyResult::RPCError })
    }
}

pub struct EarlyReplyRecv {
    recv: UnboundedReceiver<EarlyReply>,
}

impl EarlyReplyRecv {
    pub async fn recv(&mut self) -> Option<EarlyReply> {
        self.recv.recv().await
    }
}

pub struct ReplyRecv {
    recv: UnboundedReceiver<Reply>,
}

impl ReplyRecv {
    pub async fn recv(&mut self) -> Option<Reply> {
        self.recv.recv().await
    }
}

pub struct ReplySender {
    sender: UnboundedSender<Reply>,
}

impl ReplySender {
    pub async fn send_ok_reply(&self, xid: u32, res: ProcResOk) -> Result<(), SendError<Reply>> {
        self.sender.send(Reply { xid, result: ProcResult::Ok(res) })
    }

    pub async fn send_error_reply(&self, xid: u32, err: ProcError) -> Result<(), SendError<Reply>> {
        self.sender.send(Reply { xid, result: ProcResult::Error(err) })
    }
}

pub struct ProcSender {
    sender: UnboundedSender<Procedure>,
}

impl ProcSender {
    pub async fn send_nfsv3(&self, xid: u32) -> Result<(), SendError<Procedure>> {
        self.sender.send(Procedure { xid, args: Command::NFSv3Command })
    }
    pub async fn send_mount(&self, xid: u32) -> Result<(), SendError<Procedure>> {
        self.sender.send(Procedure { xid, args: Command::MountCommand })
    }
}

pub struct ProcRecv {
    recv: UnboundedReceiver<Procedure>,
}

impl ProcRecv {
    pub async fn recv(&mut self) -> Option<Procedure> {
        self.recv.recv().await
    }
}

pub fn create_proc_channel() -> (ProcSender, ProcRecv) {
    let (sender, recv) = mpsc::unbounded_channel::<Procedure>();
    (ProcSender { sender }, ProcRecv { recv })
}

pub fn create_reply_channel() -> (ReplySender, ReplyRecv) {
    let (sender, recv) = mpsc::unbounded_channel::<Reply>();
    (ReplySender { sender }, ReplyRecv { recv })
}

pub fn create_early_reply_channel() -> (EarlyReplySender, EarlyReplyRecv) {
    let (sender, recv) = mpsc::unbounded_channel::<EarlyReply>();
    (EarlyReplySender { sender }, EarlyReplyRecv { recv })
}
