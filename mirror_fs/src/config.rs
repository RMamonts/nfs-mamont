use std::num::NonZeroUsize;
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;

const DEFAULT_VFS_POOL_SIZE: usize = 10;
const MAX_EXPORTS_COUNT: usize = 256;
const DEFAULT_BUFFER_SIZE: usize = 64 * 1024;
const DEFAULT_BUFFER_COUNT: usize = 2048;

#[derive(Debug)]
pub struct Config {
    pub allocator: AllocatorConfig,
    pub vfs_pool_size: NonZeroUsize,
    pub export_root: PathBuf,
    pub exports: Vec<ExportConfig>,
}

#[derive(Debug)]
pub struct AllocatorConfig {
    pub buffer_size: NonZeroUsize,
    pub buffer_count: NonZeroUsize,
}

#[derive(Debug)]
pub struct ExportConfig {
    pub local_path: PathBuf,
    pub mount_path: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            allocator: AllocatorConfig::default(),
            vfs_pool_size: NonZeroUsize::new(DEFAULT_VFS_POOL_SIZE).unwrap(),
            export_root: PathBuf::new(),
            exports: Vec::with_capacity(MAX_EXPORTS_COUNT),
        }
    }
}

impl Default for AllocatorConfig {
    fn default() -> Self {
        Self {
            buffer_size: NonZeroUsize::new(DEFAULT_BUFFER_SIZE).unwrap(),
            buffer_count: NonZeroUsize::new(DEFAULT_BUFFER_COUNT).unwrap(),
        }
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

    let allocator = match raw_config.allocator {
        Some(raw_alloc) => AllocatorConfig {
            buffer_size: non_zero(
                raw_alloc.buffer_size.unwrap_or(DEFAULT_BUFFER_SIZE),
                "buffer_size",
            )?,
            buffer_count: non_zero(
                raw_alloc.buffer_count.unwrap_or(DEFAULT_BUFFER_COUNT),
                "buffer_count",
            )?,
        },
        None => AllocatorConfig::default(),
    };

    let vfs_pool_size =
        non_zero(raw_config.vfs_pool_size.unwrap_or(DEFAULT_VFS_POOL_SIZE), "vfs_pool_size")?;

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

    Ok(Config { allocator, vfs_pool_size, export_root: root, exports })
}

// Unknown keys are rejected so that a config written for an older layout
// (`read_buffer_size` / `write_buffer_size`) fails loudly instead of silently
// falling back to the defaults below.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    allocator: Option<RawAllocatorConfig>,
    vfs_pool_size: Option<usize>,
    exports: Option<RawExportsConfig>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAllocatorConfig {
    buffer_size: Option<usize>,
    buffer_count: Option<usize>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
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

fn non_zero(value: usize, field: &str) -> std::io::Result<NonZeroUsize> {
    NonZeroUsize::new(value)
        .ok_or_else(|| invalid_input(format!("{field} must be greater than zero")))
}

#[cfg(test)]
mod tests {
    use super::{RawAllocatorConfig, RawConfig};

    /// The example config must keep parsing after any key rename.
    #[test]
    fn parses_current_allocator_keys() {
        let raw: RawAllocatorConfig =
            toml::from_str("buffer_size = 1024\nbuffer_count = 8\n").unwrap();

        assert_eq!(raw.buffer_size, Some(1024));
        assert_eq!(raw.buffer_count, Some(8));
    }

    /// A config written for the old split read/write pools must fail loudly
    /// instead of silently falling back to the built-in defaults.
    #[test]
    fn rejects_legacy_allocator_keys() {
        let legacy = "[allocator]\n\
             read_buffer_size = 1048576\n\
             read_buffer_count = 2048\n\
             write_buffer_size = 1048576\n\
             write_buffer_count = 2048\n";

        let Err(error) = toml::from_str::<RawConfig>(legacy) else {
            panic!("legacy allocator keys must be rejected");
        };
        assert!(
            error.to_string().contains("unknown field"),
            "error should point at the stale key, got: {error}"
        );
    }

    #[test]
    fn rejects_unknown_top_level_keys() {
        assert!(toml::from_str::<RawConfig>("vfs_poll_size = 4\n").is_err());
    }
}
