use std::io::Read;

use crate::{protocol::nfs::v4::operations, xdr};

#[derive(Debug, Default, Clone, Copy)]
pub struct Args {}

impl Args {
    pub fn execute(&self) -> Result<super::Response, anyhow::Error> {
        Ok(super::Response {
            status: operations::Status::Ok,
            resop: operations::OpNum::Null,
            uin: operations::Data::Null(Resp {}),
        })
    }
}

impl xdr::Deserialize for Args {
    fn deserialize<R: Read>(&mut self, _src: &mut R) -> std::io::Result<()> {
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct Resp {}

impl xdr::Serialize for Resp {
    fn serialize<W: std::io::Write>(&self, _dest: &mut W) -> std::io::Result<()> {
        Ok(())
    }
}
