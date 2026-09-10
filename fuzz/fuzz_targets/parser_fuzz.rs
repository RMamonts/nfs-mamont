#![no_main]

use std::sync::{Arc, OnceLock};

use libfuzzer_sys::fuzz_target;
use tokio::runtime::Runtime;
use tokio::sync::Mutex;

use fuzz_shared::{
    parser_wrapper::{ParserWrapper, RpcRequest},
    read_socket::FuzzMockSocket,
    ZeroAllocator,
};
use nfs_mamont::{
    Error, NfsArguments, ProcArguments, RpcBody, RpcParser, NFS_PROGRAM, NFS_VERSION, NULL,
    RPC_VERSION,
};

static RUNTIME: OnceLock<Runtime> = OnceLock::new();
static PARSER: OnceLock<Mutex<ParserWrapper>> = OnceLock::new();

fn get_runtime() -> &'static Runtime {
    RUNTIME.get_or_init(|| Runtime::new().unwrap())
}

fn get_parser() -> &'static Mutex<ParserWrapper> {
    PARSER.get_or_init(|| {
        let (sock, hand) = FuzzMockSocket::new();
        let mut parser =
            ParserWrapper::new(RpcParser::new(sock, Arc::new(ZeroAllocator::new())), hand);
        let initial_value = RpcRequest {
            xid: 78,
            request: RpcBody::Call as u32,
            rpc_version: RPC_VERSION,
            prog: NFS_PROGRAM,
            version: NFS_VERSION,
            proc: NULL,
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
