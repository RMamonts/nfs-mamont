#![no_main]

mod parser_wrapper;

use arbitrary::Unstructured;
use libfuzzer_sys::fuzz_target;
use nfs_mamont::allocator::TEST_SIZE;
use nfs_mamont::mocks::alloc::MockAllocator;
use nfs_mamont::mocks::fuzz_socket::FuzzMockSocket;
use nfs_mamont::parser::parser_struct::RpcParser;
use nfs_mamont::rpc::Error;
use parser_wrapper::ParserWrapper;
use std::sync::OnceLock;
use tokio::runtime::Runtime;
use tokio::sync::Mutex;

use crate::parser_wrapper::RpcRequest;

static RUNTIME: OnceLock<Runtime> = OnceLock::new();
static PARSER: OnceLock<Mutex<ParserWrapper<MockAllocator>>> = OnceLock::new();

fn get_runtime() -> &'static Runtime {
    RUNTIME.get_or_init(|| Runtime::new().unwrap())
}

fn get_parser() -> &'static Mutex<ParserWrapper<MockAllocator>> {
    PARSER.get_or_init(|| {
        let (sock, hand) = FuzzMockSocket::new();
        let parser = ParserWrapper::new(RpcParser::new(sock, MockAllocator::new(TEST_SIZE)), hand);
        Mutex::new(parser)
    })
}

fuzz_target!(|data: RpcRequest| {
    // fuzzed code goes here
    let runtime = get_runtime();

    runtime.block_on(async {
        let mut parser = get_parser().lock().await;
        parser.write_new_message(data.clone());
        match parser.parse_message().await {
            Ok(res) => {
                assert_eq!(*res, data.args);
            }
            Err(error) => match error {
                Error::RpcVersionMismatch(_)
                | Error::AuthError(_)
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
