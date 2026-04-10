use std::fs;
use std::net::SocketAddr;

use tempfile::tempdir;

#[test]
fn load_runtime_config_supports_multiple_exports() {
    let export_root = tempdir().unwrap();
    fs::create_dir_all(export_root.path().join("fs1")).unwrap();
    fs::create_dir_all(export_root.path().join("nested/fs2")).unwrap();
    let config_dir = tempdir().unwrap();
    let config_path = config_dir.path().join("mirrorfs.toml");
    fs::write(
        &config_path,
        format!(
            r#"[listen]
addr = "127.0.0.1:3049"

[allocator]
read_buffer_size = 65536
read_buffer_count = 16
write_buffer_size = 131072
write_buffer_count = 8

[exports]
root = "{}"
paths = ["fs1", "nested/fs2"]
"#,
            export_root.path().display(),
        ),
    )
    .unwrap();

    let config = crate::config::load_runtime_config(&config_path).unwrap();
    assert_eq!(config.bind, "127.0.0.1:3049".parse::<SocketAddr>().unwrap());
    assert_eq!(config.allocator.read_buffer_size.get(), 65536);
    assert_eq!(config.allocator.read_buffer_count.get(), 16);
    assert_eq!(config.allocator.write_buffer_size.get(), 131072);
    assert_eq!(config.allocator.write_buffer_count.get(), 8);
    assert_eq!(config.exports.len(), 2);
    assert_eq!(config.exports[0].mount_path, "/fs1");
    assert_eq!(
        config.exports[0].local_path,
        export_root.path().join("fs1").canonicalize().unwrap()
    );
    assert_eq!(config.exports[1].mount_path, "/nested/fs2");
    assert_eq!(
        config.exports[1].local_path,
        export_root.path().join("nested/fs2").canonicalize().unwrap()
    );
}

#[test]
fn load_runtime_config_rejects_non_relative_export_path() {
    let export_root = tempdir().unwrap();
    let nested = export_root.path().join("nested");
    fs::create_dir_all(&nested).unwrap();
    let config_dir = tempdir().unwrap();
    let config_path = config_dir.path().join("mirrorfs.toml");
    fs::write(
        &config_path,
        format!(
            r#"[exports]
root = "{}"
paths = [".", "nested"]
"#,
            export_root.path().display(),
        ),
    )
    .unwrap();

    let error = crate::config::load_runtime_config(&config_path).unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    assert!(error.to_string().contains("export path must not be empty"));
}
