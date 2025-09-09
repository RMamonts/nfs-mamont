use std::fs::OpenOptions;
use std::path::Path;
use std::process::Child;
use std::process::Command;
use std::time::Duration;

use tempfile;

fn run_demo_fs() -> Child {
    Command::new("cargo")
        .arg("run")
        .arg("--example")
        .arg("demofs")
        .spawn()
        .expect("cargo to spawn normally")
}

fn mount(path: impl AsRef<Path>) -> Child {
    let path = std::str::from_utf8(path.as_ref().as_os_str().as_encoded_bytes())
        .expect("Path to be utf8 encoded");
    Command::new("sudo")
        .args([
            "mount",
            "-t",
            "nfs",
            "-o",
            "proto=tcp,port=11111,mountport=11111,nolock,addr=127.0.0.1",
            "127.0.0.1:/",
            path,
        ])
        .spawn()
        .expect("mount to spawn normally")
}

fn umount(path: impl AsRef<Path>) -> Child {
    let path = std::str::from_utf8(path.as_ref().as_os_str().as_encoded_bytes())
        .expect("Path to be utf8 encoded");
    Command::new("sudo").args(["umount", path]).spawn().expect("umount to spawn normally")
}

// const MOUNT_POINT: &str = "/home/ierin/github/nfs-mamont/exports";

fn main() {
    let temp_dir = tempfile::tempdir().unwrap();
    let mut demo_fs = run_demo_fs();

    std::thread::sleep(Duration::from_secs(3));

    assert!(mount(&temp_dir).wait().unwrap().success());

    // let mut base = temp_dir.as_ref().to_owned();
    // let file_path = {
    //     base.push("new_file");
    //     base
    // };
    // let _file = OpenOptions::new().truncate(true).write(true).create_new(true).open(file_path).unwrap();
    // std::fs::(&format!("{temp_dir:?}/new_dir")).unwrap();;

    // assert!(umount(&temp_dir).wait().unwrap().success());

    demo_fs.kill().unwrap();
    // demo_fs.wait().unwrap();
}
