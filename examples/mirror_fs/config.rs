use std::fmt::Formatter;
use std::num::NonZeroUsize;
use std::path::{Component, Path, PathBuf};
use std::thread;

use serde::Deserialize;

const DEFAULT_VFS_POOL_SIZE: usize = 10;
const MAX_EXPORTS_COUNT: usize = 256;

pub struct Config {
    pub allocator: AllocatorConfig,
    pub disk_io: DiskIoConfig,
    pub read_path: ReadPathConfig,
    pub vfs_pool_size: NonZeroUsize,
    pub exports: Vec<ExportConfig>,
}

pub struct AllocatorConfig {
    pub read_buffer_size: NonZeroUsize,
    pub read_buffer_count: NonZeroUsize,
    pub write_buffer_size: NonZeroUsize,
    pub write_buffer_count: NonZeroUsize,
}

impl AllocatorConfig {
    const DEFAULT_READ_BUFFER_SIZE: usize = crate::fs::READ_WRITE_MAX as usize;
    const DEFAULT_READ_BUFFER_COUNT: usize = 2048;
    const DEFAULT_WRITE_BUFFER_SIZE: usize = crate::fs::READ_WRITE_MAX as usize;
    const DEFAULT_WRITE_BUFFER_COUNT: usize = 2048;
}

pub struct ExportConfig {
    pub local_path: PathBuf,
    pub mount_path: String,
}

#[derive(Clone)]
pub struct DiskIoConfig {
    pub worker_count: NonZeroUsize,
    pub ring_entries: u32,
    pub max_inflight_per_worker: NonZeroUsize,
    pub channel_capacity: NonZeroUsize,
    pub prefetch_budget_per_worker: NonZeroUsize,
    pub enable_fixed_files: bool,
}

#[derive(Clone)]
pub struct ReadPathConfig {
    pub small_io_threshold: NonZeroUsize,
    pub read_ahead_trigger_bytes: NonZeroUsize,
    pub read_ahead_window_blocks: NonZeroUsize,
    pub read_ahead_per_file_limit: NonZeroUsize,
    pub sequential_detection_window_ms: NonZeroUsize,
    pub sendfile_min_bytes: NonZeroUsize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            allocator: AllocatorConfig::default(),
            disk_io: DiskIoConfig::default(),
            read_path: ReadPathConfig::default(),
            vfs_pool_size: NonZeroUsize::new(DEFAULT_VFS_POOL_SIZE).unwrap(),
            exports: Vec::with_capacity(MAX_EXPORTS_COUNT),
        }
    }
}

impl Default for AllocatorConfig {
    fn default() -> Self {
        Self {
            read_buffer_size: NonZeroUsize::new(AllocatorConfig::DEFAULT_READ_BUFFER_SIZE).unwrap(),
            read_buffer_count: NonZeroUsize::new(AllocatorConfig::DEFAULT_READ_BUFFER_COUNT)
                .unwrap(),
            write_buffer_size: NonZeroUsize::new(AllocatorConfig::DEFAULT_WRITE_BUFFER_SIZE)
                .unwrap(),
            write_buffer_count: NonZeroUsize::new(AllocatorConfig::DEFAULT_WRITE_BUFFER_COUNT)
                .unwrap(),
        }
    }
}

impl Default for DiskIoConfig {
    fn default() -> Self {
        let worker_count =
            thread::available_parallelism().map_or(4, usize::from).clamp(2, 8);
        Self {
            worker_count: NonZeroUsize::new(worker_count).unwrap(),
            ring_entries: 256,
            max_inflight_per_worker: NonZeroUsize::new(512).unwrap(),
            channel_capacity: NonZeroUsize::new(1024).unwrap(),
            prefetch_budget_per_worker: NonZeroUsize::new(32).unwrap(),
            enable_fixed_files: false,
        }
    }
}

impl Default for ReadPathConfig {
    fn default() -> Self {
        Self {
            small_io_threshold: NonZeroUsize::new(32 * 1024).unwrap(),
            read_ahead_trigger_bytes: NonZeroUsize::new(256 * 1024).unwrap(),
            read_ahead_window_blocks: NonZeroUsize::new(8).unwrap(),
            read_ahead_per_file_limit: NonZeroUsize::new(16).unwrap(),
            sequential_detection_window_ms: NonZeroUsize::new(6_000).unwrap(),
            sendfile_min_bytes: NonZeroUsize::new(32 * 1024).unwrap(),
        }
    }
}

impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("allocator", &self.allocator)
            .field("disk_io", &self.disk_io)
            .field("read_path", &self.read_path)
            .field("vfs_pool_size", &self.vfs_pool_size)
            .field("exports", &self.exports)
            .finish()
    }
}

impl std::fmt::Debug for AllocatorConfig {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AllocatorConfig")
            .field("read_buffer_size", &self.read_buffer_size)
            .field("read_buffer_count", &self.read_buffer_count)
            .field("write_buffer_size", &self.write_buffer_size)
            .field("write_buffer_count", &self.write_buffer_count)
            .finish()
    }
}

impl std::fmt::Debug for DiskIoConfig {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DiskIoConfig")
            .field("worker_count", &self.worker_count)
            .field("ring_entries", &self.ring_entries)
            .field("max_inflight_per_worker", &self.max_inflight_per_worker)
            .field("channel_capacity", &self.channel_capacity)
            .field("prefetch_budget_per_worker", &self.prefetch_budget_per_worker)
            .field("enable_fixed_files", &self.enable_fixed_files)
            .finish()
    }
}

impl std::fmt::Debug for ReadPathConfig {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReadPathConfig")
            .field("small_io_threshold", &self.small_io_threshold)
            .field("read_ahead_trigger_bytes", &self.read_ahead_trigger_bytes)
            .field("read_ahead_window_blocks", &self.read_ahead_window_blocks)
            .field("read_ahead_per_file_limit", &self.read_ahead_per_file_limit)
            .field("sequential_detection_window_ms", &self.sequential_detection_window_ms)
            .field("sendfile_min_bytes", &self.sendfile_min_bytes)
            .finish()
    }
}

impl std::fmt::Debug for ExportConfig {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExportConfig")
            .field("local_path", &self.local_path)
            .field("mount_path", &self.mount_path)
            .finish()
    }
}

pub fn load_config(path: &Path) -> std::io::Result<Config> {
    let raw = std::fs::read_to_string(path)?;
    let raw_config: RawConfig = toml::from_str(&raw).map_err(|error| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("failed to parse config {}: {error}", path.display()),
        )
    })?;

    let defaults = AllocatorConfig::default();
    let disk_io_defaults = DiskIoConfig::default();
    let read_path_defaults = ReadPathConfig::default();
    let allocator = match raw_config.allocator {
        Some(raw_alloc) => AllocatorConfig {
            read_buffer_size: raw_alloc.read_buffer_size.unwrap_or(defaults.read_buffer_size),
            read_buffer_count: raw_alloc.read_buffer_count.unwrap_or(defaults.read_buffer_count),
            write_buffer_size: raw_alloc.write_buffer_size.unwrap_or(defaults.write_buffer_size),
            write_buffer_count: raw_alloc.write_buffer_count.unwrap_or(defaults.write_buffer_count),
        },
        None => defaults,
    };

    let disk_io = match raw_config.disk_io {
        Some(raw_disk_io) => DiskIoConfig {
            worker_count: raw_disk_io.worker_count.unwrap_or(disk_io_defaults.worker_count),
            ring_entries: raw_disk_io.ring_entries.unwrap_or(disk_io_defaults.ring_entries),
            max_inflight_per_worker: raw_disk_io
                .max_inflight_per_worker
                .unwrap_or(disk_io_defaults.max_inflight_per_worker),
            channel_capacity: raw_disk_io
                .channel_capacity
                .unwrap_or(disk_io_defaults.channel_capacity),
            prefetch_budget_per_worker: raw_disk_io
                .prefetch_budget_per_worker
                .unwrap_or(disk_io_defaults.prefetch_budget_per_worker),
            enable_fixed_files: raw_disk_io
                .enable_fixed_files
                .unwrap_or(disk_io_defaults.enable_fixed_files),
        },
        None => disk_io_defaults,
    };

    let read_path = match raw_config.read_path {
        Some(raw_read_path) => ReadPathConfig {
            small_io_threshold: raw_read_path
                .small_io_threshold
                .unwrap_or(read_path_defaults.small_io_threshold),
            read_ahead_trigger_bytes: raw_read_path
                .read_ahead_trigger_bytes
                .unwrap_or(read_path_defaults.read_ahead_trigger_bytes),
            read_ahead_window_blocks: raw_read_path
                .read_ahead_window_blocks
                .unwrap_or(read_path_defaults.read_ahead_window_blocks),
            read_ahead_per_file_limit: raw_read_path
                .read_ahead_per_file_limit
                .unwrap_or(read_path_defaults.read_ahead_per_file_limit),
            sequential_detection_window_ms: raw_read_path
                .sequential_detection_window_ms
                .unwrap_or(read_path_defaults.sequential_detection_window_ms),
            sendfile_min_bytes: raw_read_path
                .sendfile_min_bytes
                .unwrap_or(read_path_defaults.sendfile_min_bytes),
        },
        None => read_path_defaults,
    };

    let vfs_pool_size = raw_config
        .vfs_pool_size
        .unwrap_or_else(|| NonZeroUsize::new(DEFAULT_VFS_POOL_SIZE).unwrap());

    let raw_exports = raw_config
        .exports
        .ok_or_else(|| invalid_input("config must contain an [exports] section"))?;

    if raw_exports.paths.is_empty() {
        return Err(invalid_input("config must contain at least one export"));
    }
    if raw_exports.paths.len() > MAX_EXPORTS_COUNT {
        return Err(invalid_input(format!(
            "config supports at most {} exports",
            MAX_EXPORTS_COUNT
        )));
    }

    let root = resolve_export_root(&raw_exports.root)?;
    let mut exports = Vec::with_capacity(raw_exports.paths.len());
    for export_path in &raw_exports.paths {
        let relative = normalize_export_path(export_path)?;
        let local_path = resolve_export_root(&root.join(&relative))?;
        let mount_path = mount_path_for_export(&relative);
        exports.push(ExportConfig { local_path, mount_path });
    }

    validate_exports(&exports)?;

    Ok(Config { allocator, disk_io, read_path, vfs_pool_size, exports })
}

#[derive(Deserialize)]
struct RawConfig {
    allocator: Option<RawAllocatorConfig>,
    disk_io: Option<RawDiskIoConfig>,
    read_path: Option<RawReadPathConfig>,
    vfs_pool_size: Option<NonZeroUsize>,
    exports: Option<RawExportsConfig>,
}

#[derive(Deserialize)]
struct RawAllocatorConfig {
    #[serde(deserialize_with = "dehumansize_nonzero", default)]
    read_buffer_size: Option<NonZeroUsize>,
    #[serde(deserialize_with = "dehumansize_nonzero", default)]
    read_buffer_count: Option<NonZeroUsize>,
    #[serde(deserialize_with = "dehumansize_nonzero", default)]
    write_buffer_size: Option<NonZeroUsize>,
    #[serde(deserialize_with = "dehumansize_nonzero", default)]
    write_buffer_count: Option<NonZeroUsize>,
}

#[derive(Deserialize)]
struct RawDiskIoConfig {
    #[serde(deserialize_with = "dehumansize_nonzero", default)]
    worker_count: Option<NonZeroUsize>,
    ring_entries: Option<u32>,
    #[serde(deserialize_with = "dehumansize_nonzero", default)]
    max_inflight_per_worker: Option<NonZeroUsize>,
    #[serde(deserialize_with = "dehumansize_nonzero", default)]
    channel_capacity: Option<NonZeroUsize>,
    #[serde(deserialize_with = "dehumansize_nonzero", default)]
    prefetch_budget_per_worker: Option<NonZeroUsize>,
    enable_fixed_files: Option<bool>,
}

#[derive(Deserialize)]
struct RawReadPathConfig {
    #[serde(deserialize_with = "dehumansize_nonzero", default)]
    small_io_threshold: Option<NonZeroUsize>,
    #[serde(deserialize_with = "dehumansize_nonzero", default)]
    read_ahead_trigger_bytes: Option<NonZeroUsize>,
    #[serde(deserialize_with = "dehumansize_nonzero", default)]
    read_ahead_window_blocks: Option<NonZeroUsize>,
    #[serde(deserialize_with = "dehumansize_nonzero", default)]
    read_ahead_per_file_limit: Option<NonZeroUsize>,
    #[serde(deserialize_with = "dehumansize_nonzero", default)]
    sequential_detection_window_ms: Option<NonZeroUsize>,
    #[serde(deserialize_with = "dehumansize_nonzero", default)]
    sendfile_min_bytes: Option<NonZeroUsize>,
}

#[derive(Deserialize)]
struct RawExportsConfig {
    root: PathBuf,
    paths: Vec<PathBuf>,
}

fn validate_exports(exports: &[ExportConfig]) -> std::io::Result<()> {
    let mut mount_paths = std::collections::HashSet::new();
    for export in exports {
        if !mount_paths.insert(export.mount_path.clone()) {
            return Err(invalid_input(format!("duplicate mount path {}", export.mount_path)));
        }
    }

    for (index, export) in exports.iter().enumerate() {
        for other in exports.iter().skip(index + 1) {
            let overlaps = export.local_path == other.local_path
                || export.local_path.starts_with(&other.local_path)
                || other.local_path.starts_with(&export.local_path);
            if overlaps {
                return Err(invalid_input(format!(
                    "export roots must not overlap: {} and {}",
                    export.local_path.display(),
                    other.local_path.display()
                )));
            }
        }
    }

    Ok(())
}

fn resolve_export_root(path: &Path) -> std::io::Result<PathBuf> {
    let export_root = std::fs::canonicalize(path).map_err(|error| {
        std::io::Error::new(
            error.kind(),
            format!("failed to resolve export root {}: {error}", path.display()),
        )
    })?;
    let metadata = std::fs::metadata(&export_root).map_err(|error| {
        std::io::Error::new(
            error.kind(),
            format!("failed to stat export root {}: {error}", export_root.display()),
        )
    })?;
    if !metadata.is_dir() {
        return Err(invalid_input(format!(
            "export root {} must be a directory",
            export_root.display()
        )));
    }
    Ok(export_root)
}

fn normalize_export_path(path: &Path) -> std::io::Result<PathBuf> {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(value) => normalized.push(value),
            Component::CurDir => {}
            Component::RootDir | Component::ParentDir | Component::Prefix(_) => {
                return Err(invalid_input(format!(
                    "export path {} must be a relative child path without '..'",
                    path.display()
                )));
            }
        }
    }

    if normalized.as_os_str().is_empty() {
        return Err(invalid_input("export path must not be empty"));
    }

    Ok(normalized)
}

fn mount_path_for_export(path: &Path) -> String {
    let segments = path
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Vec<_>>();
    format!("/{}", segments.join("/"))
}

fn invalid_input(message: impl Into<String>) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidInput, message.into())
}

fn parse_humansize(value: &str) -> Result<NonZeroUsize, String> {
    let value = value.trim();
    if value.is_empty() {
        return Err("empty size string".to_string());
    }

    let (digits, suffix) = match value.find(|c: char| !c.is_ascii_digit()) {
        Some(pos) => (&value[..pos], &value[pos..]),
        None => (value, ""),
    };

    let base: usize = digits.parse().map_err(|e| format!("invalid number: {e}"))?;

    let multiplier: usize = match suffix.to_ascii_lowercase().as_str() {
        "" => 1,
        "k" => 1024,
        "m" => 1024 * 1024,
        "g" => 1024 * 1024 * 1024,
        "t" => 1024 * 1024 * 1024 * 1024,
        _ => return Err(format!("unknown size suffix: {suffix}")),
    };

    let total = base.checked_mul(multiplier).ok_or_else(|| "size overflow".to_string())?;

    NonZeroUsize::new(total).ok_or_else(|| "size must be greater than zero".to_string())
}

/// Deserializes positive values represented either as strings in human-readable format or as numbers.
///
/// Supports two data types:
///
/// 1) String: A non-negative decimal integer with an optional single-letter unit suffix:
///    - 'k' or 'K' for KiB (1024 bytes)
///    - 'm' or 'M' for MiB (1024 KiB)
///    - 'g' or 'G' for GiB (1024 MiB)
///    - 't' or 'T' for TiB (1024 GiB)
///
///    Examples: "10240K", "10M", "5g"
///
/// 2) Unsigned integer: The size directly as a number of bytes.
fn dehumansize_nonzero<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<NonZeroUsize>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct Visitor;

    impl serde::de::Visitor<'_> for Visitor {
        type Value = Option<NonZeroUsize>;

        fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
            formatter.write_str("a non-zero string or a non-zero unsigned integer")
        }

        fn visit_i64<E>(self, value: i64) -> std::result::Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            if value > 0 {
                Ok(Some(NonZeroUsize::new(value as usize).unwrap()))
            } else {
                Err(E::custom("expected positive integer"))
            }
        }

        fn visit_u64<E>(self, value: u64) -> std::result::Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            NonZeroUsize::new(value as usize)
                .map(Some)
                .ok_or_else(|| E::custom("expected non-zero integer"))
        }

        fn visit_str<E>(self, value: &str) -> std::result::Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            parse_humansize(value).map(Some).map_err(E::custom)
        }
    }

    deserializer.deserialize_any(Visitor)
}
