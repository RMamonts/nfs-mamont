//! NFS version 4 operation XDR definitions
//!
//! This module contains XDR definitions for NFSv4 operations
//! as specified in RFC 7530.

use std::io::{Read, Write};

use crate::{
    utils::error::io_other,
    xdr::{Deserialize, DeserializeStruct, Serialize, SerializeStruct},
};

use super::{NFSOpNum4, NFSStat4};

/// NULL4args - NULL operation arguments (void) as defined in RFC 7530
#[derive(Debug, Default, Clone, Copy)]
pub struct NULL4args {}

impl Deserialize for NULL4args {
    fn deserialize<R: Read>(_src: &mut R) -> std::io::Result<NULL4args> {
        Ok(NULL4args {})
    }
}

/// NULL4res - NULL operation response (void) as defined in RFC 7530
#[derive(Debug, Default)]
pub struct NULL4res {}

impl Serialize for NULL4res {
    fn serialize<W: Write>(&self, _dest: &mut W) -> std::io::Result<()> {
        Ok(())
    }
}

/// COMPOUND4args - COMPOUND operation arguments as defined in RFC 7530
#[derive(Debug, Default, Clone)]
pub struct COMPOUND4args {
    pub tag: String,
    pub minorversion: u32,
    pub argarray: Vec<NFSArgOp4>,
}
DeserializeStruct!(COMPOUND4args, tag, minorversion, argarray);

/// COMPOUND4res - COMPOUND operation response as defined in RFC 7530
#[derive(Debug, Default)]
pub struct COMPOUND4res {
    pub status: NFSStat4,
    pub tag: String,
    pub resarray: Vec<NFSResOp4>,
}
SerializeStruct!(COMPOUND4res, status, tag, resarray);

/// NFS argop4 - operation argument structure
#[derive(Debug, Clone)]
pub struct NFSArgOp4 {
    pub argop: NFSOpNum4,
    pub op_data: NFSArgOp4U,
}

impl Default for NFSArgOp4 {
    fn default() -> Self {
        Self { argop: NFSOpNum4::OpIllegal, op_data: NFSArgOp4U::OpIllegal }
    }
}

impl Deserialize for NFSArgOp4 {
    fn deserialize<R: Read>(src: &mut R) -> std::io::Result<NFSArgOp4> {
        let argop = NFSOpNum4::deserialize(src)?;

        match argop {
            NFSOpNum4::OpNull => {
                let args = NULL4args::deserialize(src)?;
                Ok(NFSArgOp4 { argop, op_data: NFSArgOp4U::OpNull(args) })
            }
            NFSOpNum4::OpCompound => {
                let args = COMPOUND4args::deserialize(src)?;
                Ok(NFSArgOp4 { argop, op_data: NFSArgOp4U::OpCompound(args) })
            }
            NFSOpNum4::OpIllegal => Ok(NFSArgOp4 { argop, op_data: NFSArgOp4U::OpIllegal }),
            _ => io_other("Not implemented operation: {argop:?}"),
        }
    }
}

/// NFS resop4 - operation result structure
#[derive(Debug)]
pub struct NFSResOp4 {
    pub resop: NFSOpNum4,
    pub op_result: NFSResOp4U,
}

impl Serialize for NFSResOp4 {
    fn serialize<W: Write>(&self, dest: &mut W) -> std::io::Result<()> {
        self.resop.serialize(dest)?;
        match &self.op_result {
            NFSResOp4U::OpNull(resp) => resp.serialize(dest),
            NFSResOp4U::OpCompound(resp) => resp.serialize(dest),
            NFSResOp4U::OpIllegal(resp) => resp.serialize(dest),
        }
    }
}

/// Request arguments union - nfs_argop4_u
#[derive(Debug, Clone)]
pub enum NFSArgOp4U {
    OpNull(NULL4args),
    OpCompound(COMPOUND4args),
    OpIllegal,
}

/// Response data union - nfs_resop4_u
#[derive(Debug)]
pub enum NFSResOp4U {
    OpNull(NULL4res),
    OpCompound(COMPOUND4res),
    OpIllegal(IllegalResp),
}

/// ILLEGAL operation response
#[derive(Debug, Default)]
pub struct IllegalResp {
    pub status: NFSStat4,
}

impl Serialize for IllegalResp {
    fn serialize<W: Write>(&self, dest: &mut W) -> std::io::Result<()> {
        self.status.serialize(dest)
    }
}
