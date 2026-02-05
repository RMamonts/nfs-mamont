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
static PARSER: OnceLock<Mutex<ParserWrapper<MockAllocator, FuzzMockSocket>>> = OnceLock::new();

fn get_runtime() -> &'static Runtime {
    RUNTIME.get_or_init(|| Runtime::new().unwrap())
}

fn get_parser() -> &'static Mutex<ParserWrapper<MockAllocator, FuzzMockSocket>> {
    PARSER.get_or_init(|| {
        Mutex::new(ParserWrapper::new(RpcParser::new(
            FuzzMockSocket::new(),
            MockAllocator::new(1000),
            MAX_MESSAGE_LEN,
        )))
    })
}

//на данный момент я сотворил какую-то хрень
//мне не нужно напрямую писать в буферы, мне нужно сделать так,
//чтобы сокет хранил байты нового сообщения и умел отвечать на async read
//проблема в том, что черех парсер невозможно обратиться к сокету,
//чтобы туда что-то записать (потому что не на каждый вызов чтения нужно получать новое сообщение) и при этом нельзя поменять его так, чтобы он сам
//делал вызовы из mpsc
//и при этом невозможно напрямую писать в сокет, который лежит в парсере, не изменяя trait bounds

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
