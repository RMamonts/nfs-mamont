mod dir;
mod file;
mod random;

mod utils;

use std::process::Command;
use std::time::Duration;

const EXPORT_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/", "exports");

#[test]
fn integration_test() {
    let mut random = random::Random::default();

    let mut demofs = Command::new("cargo").args(["run", "--example", "demofs"]).spawn().unwrap();

    // wait for mount
    std::thread::sleep(Duration::from_secs(2));

    let mut mount = Command::new("sudo")
        .args(["mount", "-t", "nfs", "127.0.0.1:/", EXPORT_DIR])
        .spawn()
        .unwrap();
    assert!(mount.wait().unwrap().success());

    eprintln!("starting file tests");
    file::create_write_read_delete(&mut random);

    eprintln!("starting dir tests");
    dir::create_read_delete();

    demofs.kill().unwrap();
    demofs.wait().unwrap();
}
