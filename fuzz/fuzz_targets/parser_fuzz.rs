#![no_main]

mod parser_wrapper;

use crate::parser_wrapper::RpcRequest;
use libfuzzer_sys::fuzz_target;
use nfs_mamont::allocator::TEST_SIZE;
use nfs_mamont::mocks::alloc::MockAllocator;
use nfs_mamont::mocks::read_socket::FuzzMockSocket;
use nfs_mamont::parser::parser_struct::RpcParser;
use nfs_mamont::parser::{NfsArguments, ProcArguments};
use nfs_mamont::rpc::{Error, RpcBody, RPC_VERSION};
use parser_wrapper::ParserWrapper;
use std::sync::{Arc, OnceLock};
use tokio::runtime::Runtime;
use tokio::sync::Mutex;

static RUNTIME: OnceLock<Runtime> = OnceLock::new();
static PARSER: OnceLock<Mutex<ParserWrapper<MockAllocator>>> = OnceLock::new();

fn get_runtime() -> &'static Runtime {
    RUNTIME.get_or_init(|| Runtime::new().unwrap())
}

fn get_parser() -> &'static Mutex<ParserWrapper<MockAllocator>> {
    PARSER.get_or_init(|| {
        let (sock, hand) = FuzzMockSocket::new();
        let mut parser =
            ParserWrapper::new(RpcParser::new(sock, Arc::new(MockAllocator::new(TEST_SIZE))), hand);
        let initial_value = RpcRequest {
            xid: 78,
            request: RpcBody::Call as u32,
            rpc_version: RPC_VERSION,
            prog: nfs_mamont::consts::nfsv3::NFS_PROGRAM,
            version: nfs_mamont::consts::nfsv3::NFS_VERSION,
            proc: nfs_mamont::consts::nfsv3::NULL,
            auth: 0,
            auth_verf: 0,
            args: ProcArguments::Nfs3(Box::new(NfsArguments::Null)),
        };
        parser.write_new_message(initial_value);
        Mutex::new(parser)
    })
}

fuzz_target!(|data: RpcRequest| {
    // fuzzed code goes here
    let runtime = get_runtime();

    runtime.block_on(async {
        let mut parser = get_parser().lock().await;
        parser.write_new_message(data);
        match parser.parse_message().await {
            Ok(_) => {}
            Err(error) => match error.error {
                Error::RpcVersionMismatch(_)
                | Error::Auth(_)
                | Error::ProgramMismatch
                | Error::ProcedureMismatch
                | Error::MessageTypeMismatch
                | Error::ProgramVersionMismatch(_) => {}
                _ => {
                    panic!("{:?}", error);
                }
            },
        }
    });
});
