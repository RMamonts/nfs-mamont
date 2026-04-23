use std::fmt::Formatter;
use std::num::NonZeroUsize;
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;

const DEFAULT_VFS_POOL_SIZE: usize = 10;
const MAX_EXPORTS_COUNT: usize = 256;

pub struct Config {
    pub allocator: AllocatorConfig,
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

impl Default for Config {
    fn default() -> Self {
        Self {
            allocator: AllocatorConfig::default(),
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

impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("allocator", &self.allocator)
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
    let allocator = match raw_config.allocator {
        Some(raw_alloc) => AllocatorConfig {
            read_buffer_size: raw_alloc.read_buffer_size.unwrap_or(defaults.read_buffer_size),
            read_buffer_count: raw_alloc.read_buffer_count.unwrap_or(defaults.read_buffer_count),
            write_buffer_size: raw_alloc.write_buffer_size.unwrap_or(defaults.write_buffer_size),
            write_buffer_count: raw_alloc.write_buffer_count.unwrap_or(defaults.write_buffer_count),
        },
        None => defaults,
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

    Ok(Config { allocator, vfs_pool_size, exports })
}

#[derive(Deserialize)]
struct RawConfig {
    allocator: Option<RawAllocatorConfig>,
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
