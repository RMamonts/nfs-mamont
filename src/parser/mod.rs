use std::future::Future;
use std::io;
use std::string::FromUtf8Error;

use crate::parser::rpc::AuthStat;

pub mod mount;
pub mod nfsv3;
mod parser;
pub mod primitive;
mod rpc;
#[cfg(test)]
mod tests;

pub type Result<T> = std::result::Result<T, Error>;

pub async fn process_suberror(error: Error, fun: impl Future<Output = Result<()>>) -> Error {
    match fun.await {
        Ok(_) => error,
        Err(err) => err,
    }
}

#[derive(Debug)]
struct ProgramVersionMismatch(u32, u32);
#[derive(Debug)]
struct RPCVersionMismatch(u32, u32);

#[derive(Debug)]
#[allow(unused)]
pub enum Error {
    MaxELemLimit,
    IO(io::Error),
    EnumDiscMismatch,
    IncorrectString(FromUtf8Error),
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
