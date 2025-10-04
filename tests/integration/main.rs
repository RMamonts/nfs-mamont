mod defer;
mod dir;
mod file;
mod random;

mod utils;

use std::process::Command;
use std::time::Duration;

use crate::defer::{defer, Defer};

const MOUNT_POINT_PATH: &str = "/mnt/mamont_mount/";

fn create_dir(path: &'static str) -> Defer<'static> {
    let mut mkdir = Command::new("sudo").args(["mkdir", path]).spawn().unwrap();
    assert!(mkdir.wait().unwrap().success());

    defer(|| {
        let mut rmdir = Command::new("sudo").args(["rmdir", path]).spawn().unwrap();
        assert!(rmdir.wait().unwrap().success());
    })
}

fn run_demofs() -> Defer<'static> {
    let mut demofs = Command::new("cargo").args(["run", "--example", "demofs"]).spawn().unwrap();

    defer(move || {
        demofs.kill().unwrap();
        demofs.wait().unwrap();
    })
}

fn mount(mount_point: &'static str) -> Defer<'static> {
    let mut mount = Command::new("sudo")
        .args([
            "mount",
            "-t",
            "nfs",
            "-o",
            "proto=tcp,port=11111,mountport=11111,nolock,addr=127.0.0.1",
            "127.0.0.1:/",
            MOUNT_POINT_PATH,
        ])
        .spawn()
        .unwrap();
    assert!(mount.wait().unwrap().success());

    defer(|| {
        let mut umount =
            Command::new("sudo").args(["umount", "-t", "nfs", mount_point]).spawn().unwrap();
        assert!(umount.wait().unwrap().success());
    })
}

#[test]
fn integration_test() {
    let _mount_point = create_dir(MOUNT_POINT_PATH);

    let _demofs = run_demofs();
    // wait for demofs to up
    std::thread::sleep(Duration::from_secs(10));

    let _mount = mount(MOUNT_POINT_PATH);

    let mut random = random::Random::default();

    file::create_write_read_delete(MOUNT_POINT_PATH, &mut random);
    dir::create_read_delete(MOUNT_POINT_PATH);
}
