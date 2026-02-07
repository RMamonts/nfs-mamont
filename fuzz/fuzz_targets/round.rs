#![no_main]

use libfuzzer_sys::fuzz_target;
use nfs_mamont::mocks::alloc::MockAllocator;
use nfs_mamont::mocks::fuzz_socket::FuzzMockSocket;
use nfs_mamont::mocks::parser_wrapper::ParserWrapper;
use nfs_mamont::parser::parser_struct::{RpcParser, MAX_MESSAGE_LEN};
use nfs_mamont::parser::Arguments;
use std::sync::OnceLock;
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
        let parser = ParserWrapper::new(
            RpcParser::new(sock, MockAllocator::new(1000), MAX_MESSAGE_LEN),
            hand,
        );
        Mutex::new(parser)
    })
}

fuzz_target!(|data: Arguments| {
    // fuzzed code goes here
    let runtime = get_runtime();

    let res = runtime.block_on(async {
        let mut parser = get_parser().lock().await;
        parser.write_new_message(data.clone());
        parser.parse_message().await
    });
    assert_eq!(*res, data);
});
