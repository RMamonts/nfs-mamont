use std::io::{Read, Write};

use crate::protocol::nfs::v4::operations;
use crate::{
    protocol::nfs::v4::operations::{OpNum, Request, Response, Status},
    xdr::{Deserialize, Serialize},
    DeserializeStruct, SerializeStruct,
};

#[derive(Debug, Default)]
pub struct Args {
    pub tag: String,
    pub minor_version: u32,
    pub arg_array: Vec<Request>,
}
DeserializeStruct!(Args, tag, minor_version, arg_array);

impl Args {
    pub fn execute(&self) -> Result<Response, anyhow::Error> {
        let mut response =
            Resp { status: Status::Ok, tag: self.tag.clone(), res_array: Vec::new() };

        for op in &self.arg_array {
            let res = match &op.uin {
                operations::Argument::Null(args) => args.execute(),
                operations::Argument::Compound(_) => {
                    response.status = Status::ErrOpIllegal;
                    return Ok(Response {
                        status: response.status,
                        resop: OpNum::Compound,
                        uin: operations::Data::Compound(response),
                    });
                }
                _ => {
                    unimplemented!()
                }
            }?;

            if res.status != operations::Status::Ok {
                response.status = res.status;
                response.res_array.push(res);
                return Ok(Response {
                    status: response.status,
                    resop: OpNum::Compound,
                    uin: operations::Data::Compound(response),
                });
            }

            response.res_array.push(res);
        }
        Ok(Response {
            status: Status::Ok,
            resop: OpNum::Compound,
            uin: operations::Data::Compound(response),
        })
    }
}

#[derive(Debug, Default)]
pub struct Resp {
    pub status: Status,
    pub tag: String,
    pub res_array: Vec<Response>,
}
SerializeStruct!(Resp, status, tag, res_array);
