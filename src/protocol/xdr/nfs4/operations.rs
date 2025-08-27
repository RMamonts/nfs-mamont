//! NFS version 4 operation XDR definitions
//!
//! This module contains XDR definitions for NFSv4 operations
//! as specified in RFC 7530.

use std::io::{Read, Write};

use crate::{utils::error::io_other, xdr::{Deserialize, DeserializeStruct, Serialize, SerializeStruct}};

use super::{nfs_opnum4, nfsstat4};

/// NULL4args - NULL operation arguments (void) as defined in RFC 7530
#[allow(non_camel_case_types)]
#[derive(Debug, Default, Clone, Copy)]
pub struct NULL4args {}

impl Deserialize for NULL4args {
    fn deserialize<R: Read>(_src: &mut R) -> std::io::Result<NULL4args> {
        Ok(NULL4args {})
    }
}

/// NULL4res - NULL operation response (void) as defined in RFC 7530
#[allow(non_camel_case_types)]
#[derive(Debug, Default)]
pub struct NULL4res {}

impl Serialize for NULL4res {
    fn serialize<W: Write>(&self, _dest: &mut W) -> std::io::Result<()> {
        Ok(())
    }
}

/// COMPOUND4args - COMPOUND operation arguments as defined in RFC 7530
#[allow(non_camel_case_types)]
#[derive(Debug, Default, Clone)]
pub struct COMPOUND4args {
    pub tag: String,
    pub minorversion: u32,
    pub argarray: Vec<nfs_argop4>,
}
DeserializeStruct!(COMPOUND4args, tag, minorversion, argarray);

/// COMPOUND4res - COMPOUND operation response as defined in RFC 7530
#[allow(non_camel_case_types)]
#[derive(Debug, Default)]
pub struct COMPOUND4res {
    pub status: nfsstat4,
    pub tag: String,
    pub resarray: Vec<nfs_resop4>,
}
SerializeStruct!(COMPOUND4res, status, tag, resarray);

/// NFS argop4 - operation argument structure
#[allow(non_camel_case_types)]
#[derive(Debug, Clone)]
pub struct nfs_argop4 {
    pub argop: nfs_opnum4,
    pub op_data: nfs_argop4_u,
}

impl Default for nfs_argop4 {
    fn default() -> Self {
        Self { argop: nfs_opnum4::OP_ILLEGAL, op_data: nfs_argop4_u::OpIllegal }
    }
}

impl Deserialize for nfs_argop4 {
    fn deserialize<R: Read>(src: &mut R) -> std::io::Result<nfs_argop4> {
        let argop = nfs_opnum4::deserialize(src)?;

        match argop {
            nfs_opnum4::OP_NULL => {
                let args = NULL4args::deserialize(src)?;
                Ok(nfs_argop4 {
                    argop,
                    op_data: nfs_argop4_u::OpNull(args),
                })
            }
            nfs_opnum4::OP_COMPOUND => {
                let args = COMPOUND4args::deserialize(src)?;
                Ok(nfs_argop4 {
                    argop,
                    op_data: nfs_argop4_u::OpCompound(args),
                })
            }
            nfs_opnum4::OP_ILLEGAL => {
                Ok(nfs_argop4 {
                    argop,
                    op_data: nfs_argop4_u::OpIllegal,
                })
            }
            _ => {
                io_other("Not implemented operation: {argop:?}")
            }
        }
    }
}

/// NFS resop4 - operation result structure
#[allow(non_camel_case_types)]
#[derive(Debug)]
pub struct nfs_resop4 {
    pub resop: nfs_opnum4,
    pub op_result: nfs_resop4_u,
}

impl Serialize for nfs_resop4 {
    fn serialize<W: Write>(&self, dest: &mut W) -> std::io::Result<()> {
        self.resop.serialize(dest)?;
        match &self.op_result {
            nfs_resop4_u::OpNull(resp) => resp.serialize(dest),
            nfs_resop4_u::OpCompound(resp) => resp.serialize(dest),
            nfs_resop4_u::OpIllegal(resp) => resp.serialize(dest),
        }
    }
}

/// Request arguments union - nfs_argop4_u
#[allow(non_camel_case_types)]
#[derive(Debug, Clone)]
pub enum nfs_argop4_u {
    OpNull(NULL4args),
    OpCompound(COMPOUND4args),
    OpIllegal,
}

/// Response data union - nfs_resop4_u  
#[allow(non_camel_case_types)]
#[derive(Debug)]
pub enum nfs_resop4_u {
    OpNull(NULL4res),
    OpCompound(COMPOUND4res),
    OpIllegal(IllegalResp),
}

/// ILLEGAL operation response
#[derive(Debug, Default)]
pub struct IllegalResp {
    pub status: nfsstat4,
}

impl Serialize for IllegalResp {
    fn serialize<W: Write>(&self, dest: &mut W) -> std::io::Result<()> {
        self.status.serialize(dest)
    }
}
