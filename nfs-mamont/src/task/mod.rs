//! Task management for NFS server operations.
//!
//! This module provides the task infrastructure for handling NFS server operations,
//! including connection-specific tasks and global task coordination.
use nfs_mamont_derive::XDRSize;

use crate::allocator::Buffer;
use crate::mount::MountRes;
use crate::nlm::NlmRes;
use crate::rpc::{Error, OpaqueAuth};
use crate::vfs::NfsRes;
use crate::xdr;

pub mod connection;
pub mod global;

/// Tagged union of top-level RPC program results supported by this server.
#[derive(XDRSize)]
pub enum ProcResult<B: Buffer> {
    Nfs3(Box<NfsRes<B>>),
    Mount(Box<MountRes>),
    Nlm4(Box<NlmRes>),
}

/// RPC reply metadata plus a typed result to be serialized.
pub struct ProcReply<B: Buffer> {
    pub xid: u32,
    pub proc_result: Result<ProcResult<B>, Error>,
}

pub struct RPCReply<B: Buffer> {
    pub xid: u32,
    pub verifier: OpaqueAuth,
    pub result: Result<ProcResult<B>, Error>,
}

impl<B: Buffer> xdr::XDRSize for RPCReply<B> {
    fn xdr_size(&self) -> usize {
        let xid = Self::INTEGER;
        let reply_marker = Self::INTEGER;
        let body = match self.result {
            Ok(ref body) => {
                let accepted_marker = Self::INTEGER;
                let auth = self.verifier.xdr_size();
                let success_marker = Self::INTEGER;
                let body = body.xdr_size();
                accepted_marker + body + auth + success_marker + body
            }
            Err(ref err) => match err {
                Error::MaxElemLimit
                | Error::EnumDiscMismatch
                | Error::IncorrectString(_)
                | Error::ImpossibleTypeCast
                | Error::BadFileHandle
                | Error::MessageTypeMismatch => {
                    let accepted_marker = Self::INTEGER;
                    let auth = self.verifier.xdr_size();
                    let garbage_marker = Self::INTEGER;
                    accepted_marker + auth + garbage_marker
                }
                Error::RpcVersionMismatch(_) => {
                    let denied_marker = Self::INTEGER;
                    let prc_mismatch = Self::INTEGER;
                    let ans = Self::INTEGER + Self::INTEGER;
                    denied_marker + prc_mismatch + ans
                }
                Error::Auth(_) => {
                    let denied_marker = Self::INTEGER;
                    let auth_err = Self::INTEGER;
                    let stat = Self::INTEGER;
                    denied_marker + auth_err + stat
                }
                Error::ProgramMismatch => {
                    let accepted_marker = Self::INTEGER;
                    let auth = self.verifier.xdr_size();
                    let prog_unavail = Self::INTEGER;
                    accepted_marker + auth + prog_unavail
                }
                Error::ProcedureMismatch => {
                    let accepted_marker = Self::INTEGER;
                    let auth = self.verifier.xdr_size();
                    let proc_unavail = Self::INTEGER;
                    accepted_marker + auth + proc_unavail
                }
                Error::ProgramVersionMismatch(_) => {
                    let accepted_marker = Self::INTEGER;
                    let auth = self.verifier.xdr_size();
                    let prog_unavail = Self::INTEGER;
                    let ans = Self::INTEGER + Self::INTEGER;
                    accepted_marker + auth + prog_unavail + ans
                }
                Error::IO(_) => {
                    let accepted_marker = Self::INTEGER;
                    let auth = self.verifier.xdr_size();
                    let sys_err = Self::INTEGER;
                    accepted_marker + auth + sys_err
                }
            },
        };
        xid + reply_marker + body
    }
}
