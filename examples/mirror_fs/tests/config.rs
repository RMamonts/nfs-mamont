use std::fs;

use tempfile::tempdir;

#[test]
fn load_config_supports_multiple_exports() {
    let export_root = tempdir().unwrap();
    fs::create_dir_all(export_root.path().join("fs1")).unwrap();
    fs::create_dir_all(export_root.path().join("nested/fs2")).unwrap();
    let config_dir = tempdir().unwrap();
    let config_path = config_dir.path().join("mirrorfs.toml");
    fs::write(
        &config_path,
        format!(
            r#"[allocator]
read_buffer_size = 65536
read_buffer_count = 16
write_buffer_size = 131072
write_buffer_count = 8

[disk_io]
worker_count = 6
ring_entries = 512
max_inflight_per_worker = 1024
channel_capacity = 2048
prefetch_budget_per_worker = 24
enable_fixed_files = true

[read_path]
small_io_threshold = 16384
read_ahead_trigger_bytes = 524288
read_ahead_window_blocks = 4
read_ahead_per_file_limit = 12
sequential_detection_window_ms = 2500
sendfile_min_bytes = 65536

[exports]
root = "{}"
paths = ["fs1", "nested/fs2"]
"#,
            export_root.path().display(),
        ),
    )
    .unwrap();

    let config = crate::config::load_config(&config_path).unwrap();
    assert_eq!(config.allocator.read_buffer_size.get(), 65536);
    assert_eq!(config.allocator.read_buffer_count.get(), 16);
    assert_eq!(config.allocator.write_buffer_size.get(), 131072);
    assert_eq!(config.allocator.write_buffer_count.get(), 8);
    assert_eq!(config.disk_io.worker_count.get(), 6);
    assert_eq!(config.disk_io.ring_entries, 512);
    assert_eq!(config.disk_io.max_inflight_per_worker.get(), 1024);
    assert_eq!(config.disk_io.channel_capacity.get(), 2048);
    assert_eq!(config.disk_io.prefetch_budget_per_worker.get(), 24);
    assert!(config.disk_io.enable_fixed_files);
    assert_eq!(config.read_path.small_io_threshold.get(), 16384);
    assert_eq!(config.read_path.read_ahead_trigger_bytes.get(), 524288);
    assert_eq!(config.read_path.read_ahead_window_blocks.get(), 4);
    assert_eq!(config.read_path.read_ahead_per_file_limit.get(), 12);
    assert_eq!(config.read_path.sequential_detection_window_ms.get(), 2500);
    assert_eq!(config.read_path.sendfile_min_bytes.get(), 65536);
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
fn load_config_rejects_non_relative_export_path() {
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

    let error = crate::config::load_config(&config_path).unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    assert!(error.to_string().contains("export path must not be empty"));
}
