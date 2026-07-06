#![no_main]

pub mod write_socket;

use std::sync::OnceLock;

use libfuzzer_sys::fuzz_target;
use tokio::runtime::Runtime;
use tokio::sync::Mutex;

use nfs_mamont::{AuthFlavor, MockBuffers, OpaqueAuth, ProcReply, Serializer};

use write_socket::MockWriter;

type TestSerializer = Serializer<MockBuffers, MockWriter>;
static RUNTIME: OnceLock<Runtime> = OnceLock::new();
static SERIALIZER: OnceLock<Mutex<TestSerializer>> = OnceLock::new();

fn get_runtime() -> &'static Runtime {
    RUNTIME.get_or_init(|| Runtime::new().unwrap())
}

fn get_serializer() -> &'static Mutex<TestSerializer> {
    SERIALIZER.get_or_init(|| Mutex::new(Serializer::new(MockWriter)))
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
