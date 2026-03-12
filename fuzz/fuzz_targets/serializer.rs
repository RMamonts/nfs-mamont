#![no_main]

use libfuzzer_sys::fuzz_target;
use nfs_mamont::mocks::write_socket::MockWriter;
use nfs_mamont::serializer::serialize_struct::{ReplyFromVfs, Serializer};
use std::sync::OnceLock;
use tokio::runtime::Runtime;
use tokio::sync::Mutex;

static RUNTIME: OnceLock<Runtime> = OnceLock::new();
static PARSER: OnceLock<Mutex<Serializer<MockWriter>>> = OnceLock::new();

fn get_runtime() -> &'static Runtime {
    RUNTIME.get_or_init(|| Runtime::new().unwrap())
}

fn get_serializer() -> &'static Mutex<Serializer<MockWriter>> {
    PARSER.get_or_init(|| Mutex::new(Serializer::new(MockWriter)))
}

fuzz_target!(|data: ReplyFromVfs| {
    // fuzzed code goes here
    let runtime = get_runtime();

    runtime.block_on(async {
        let mut ser = get_serializer().lock().await;
        ser.form_reply(data).await.unwrap();
    });
});
