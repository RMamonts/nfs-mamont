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
