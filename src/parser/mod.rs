use crate::rpc::Error;

pub mod nfsv3;
pub mod primitive;
#[cfg(test)]
mod tests;

pub type Result<T> = std::result::Result<T, Error>;
