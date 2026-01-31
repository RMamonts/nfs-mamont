use crate::rpc::AuthStat;

pub mod nfsv3;
pub mod primitive;
#[cfg(test)]
mod tests;

pub type Result<T> = std::result::Result<T, Error>;

#[allow(dead_code)]
#[derive(Debug)]
pub struct ProgramVersionMismatch {
    pub low: u32,
    pub high: u32,
}
#[allow(dead_code)]
#[derive(Debug)]
pub struct RPCVersionMismatch {
    pub low: u32,
    pub high: u32,
}
#[derive(Debug)]
#[allow(unused)]
pub enum Error {
    IncorrectPadding,
    ImpossibleTypeCast,
    BadFileHandle,
    MessageTypeMismatch,
    RpcVersionMismatch(RPCVersionMismatch),
    AuthError(AuthStat),
    ProgramMismatch,
    ProcedureMismatch,
    ProgramVersionMismatch(ProgramVersionMismatch),
}
