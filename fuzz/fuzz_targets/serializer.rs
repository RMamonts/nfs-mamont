#![no_main]

use libfuzzer_sys::fuzz_target;
use nfs_mamont::mocks::buffer::MockBuffers;
use nfs_mamont::mocks::write_socket::MockWriter;
use nfs_mamont::rpc::{AuthFlavor, OpaqueAuth};
use nfs_mamont::serializer::server::serialize_struct::Serializer;
use nfs_mamont::task::ProcReply;
use std::sync::OnceLock;
use tokio::runtime::Runtime;
use tokio::sync::Mutex;

type TestSerializer = Serializer<MockBuffers, MockWriter>;
static RUNTIME: OnceLock<Runtime> = OnceLock::new();
static PARSER: OnceLock<Mutex<TestSerializer>> = OnceLock::new();

fn get_runtime() -> &'static Runtime {
    RUNTIME.get_or_init(|| Runtime::new().unwrap())
}

fn get_serializer() -> &'static Mutex<TestSerializer> {
    PARSER.get_or_init(|| Mutex::new(Serializer::new(MockWriter)))
}

fuzz_target!(|data: ProcReply<MockBuffers>| {
    // fuzzed code goes here
    let runtime = get_runtime();
    let auth = OpaqueAuth { flavor: AuthFlavor::None, body: vec![] };
    runtime.block_on(async {
        let mut ser = get_serializer().lock().await;
        ser.form_reply(data, auth).await.unwrap();
    });
});
