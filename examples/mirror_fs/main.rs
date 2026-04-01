use std::num::NonZeroUsize;
use std::path::Component;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Deserialize;
use tokio::net::TcpListener;
use tracing::info;

use nfs_mamont::{handle_forever_with_exports, MountExport, ServerContext};

#[cfg(debug_assertions)]
use nfs_mamont::init_tracing;

use crate::fs::READ_WRITE_MAX;

pub mod fs;
pub mod fs_map;

#[cfg(test)]
mod tests;

const DEFAULT_BIND: &str = "0.0.0.0:2049";

#[derive(Debug)]
struct RuntimeConfig {
    bind: String,
    allocator: AllocatorConfig,
    exports: Vec<ConfiguredExport>,
}

#[derive(Debug)]
struct AllocatorConfig {
    read_buffer_size: usize,
    read_buffer_count: usize,
    write_buffer_size: usize,
    write_buffer_count: usize,
}

#[derive(Debug)]
struct ConfiguredExport {
    local_path: PathBuf,
    mount_path: String,
}

#[derive(Deserialize)]
struct FileConfig {
    listen: Option<FileListenConfig>,
    allocator: Option<FileAllocatorConfig>,
    exports: FileExportsConfig,
}

#[derive(Deserialize)]
struct FileListenConfig {
    addr: Option<String>,
}

#[derive(Deserialize)]
struct FileAllocatorConfig {
    read_buffer_size: Option<usize>,
    read_buffer_count: Option<usize>,
    write_buffer_size: Option<usize>,
    write_buffer_count: Option<usize>,
}

#[derive(Deserialize)]
struct FileExportsConfig {
    root: PathBuf,
    paths: Vec<PathBuf>,
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let config = parse_runtime_config(std::env::args_os())?;
    let fs = Arc::new(fs::MirrorFS::new_many(
        config.exports.iter().map(|export| export.local_path.clone()).collect(),
    ));
    let context = ServerContext::new(fs.clone(), non_zero(1024 * 1024), non_zero(1024));

    #[cfg(debug_assertions)]
    init_tracing();

    info!(bind = %config.bind, exports = config.exports.len(), "mirrorfs startup");

    let mut exports = Vec::with_capacity(config.exports.len());
    for (export_id, configured_export) in config.exports.iter().enumerate() {
        info!(
            export_root = %configured_export.local_path.display(),
            mount_path = %configured_export.mount_path,
            "configured mirror export"
        );
        let root_handle = fs.root_handle_for_export(export_id).await;
        let export =
            MountExport::from_directory_path(configured_export.mount_path.clone(), root_handle)
                .map_err(|error| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!(
                            "failed to register export {}: {error}",
                            configured_export.mount_path
                        ),
                    )
                })?;
        exports.push(export);
    }

    let listener = TcpListener::bind(&config.bind).await?;
    handle_forever_with_exports(listener, context, exports).await
}

fn parse_runtime_config(
    args: impl IntoIterator<Item = std::ffi::OsString>,
) -> std::io::Result<RuntimeConfig> {
    let mut args = args.into_iter();
    let binary_name = args
        .next()
        .unwrap_or_else(|| std::ffi::OsString::from("mirrorfs"))
        .to_string_lossy()
        .into_owned();
    let Some(first_arg) = args.next() else {
        return Err(invalid_input(format!(
            "usage: {binary_name} <directory> [bind] | {binary_name} --config <file.toml>"
        )));
    };

    if first_arg == std::ffi::OsStr::new("--config") {
        let config_path = args
            .next()
            .ok_or_else(|| invalid_input(format!("usage: {binary_name} --config <file.toml>")))?;
        if args.next().is_some() {
            return Err(invalid_input("unexpected extra arguments after config path"));
        }
        return load_runtime_config(Path::new(&config_path));
    }

    let bind = match args.next() {
        Some(value) => os_string_into_string(value, "bind address")?,
        None => DEFAULT_BIND.to_string(),
    };
    if args.next().is_some() {
        return Err(invalid_input("unexpected extra arguments"));
    }

    let export_root = resolve_export_root(Path::new(&first_arg))?;
    Ok(RuntimeConfig {
        bind,
        allocator: AllocatorConfig::default(),
        exports: vec![ConfiguredExport {
            local_path: export_root.clone(),
            mount_path: export_root.to_string_lossy().into_owned(),
        }],
    })
}

fn load_runtime_config(path: &Path) -> std::io::Result<RuntimeConfig> {
    let raw = std::fs::read_to_string(path)?;
    let config: FileConfig = toml::from_str(&raw).map_err(|error| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("failed to parse config {}: {error}", path.display()),
        )
    })?;
    if config.exports.paths.is_empty() {
        return Err(invalid_input("config must contain at least one export"));
    }
    if config.exports.paths.len() > 256 {
        return Err(invalid_input("config supports at most 256 exports"));
    }

    let root = resolve_export_root(&config.exports.root)?;
    let mut exports = Vec::with_capacity(config.exports.paths.len());
    for export_path in config.exports.paths {
        let relative_export_path = normalize_export_path(&export_path)?;
        let local_path = resolve_export_root(&root.join(&relative_export_path))?;
        let mount_path = mount_path_for_export(&relative_export_path);
        exports.push(ConfiguredExport { local_path, mount_path });
    }

    validate_exports(&exports)?;
    Ok(RuntimeConfig {
        bind: config
            .listen
            .and_then(|listen| listen.addr)
            .unwrap_or_else(|| DEFAULT_BIND.to_string()),
        allocator: AllocatorConfig::from_file_config(config.allocator)?,
        exports,
    })
}

fn validate_exports(exports: &[ConfiguredExport]) -> std::io::Result<()> {
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

fn os_string_into_string(value: std::ffi::OsString, field_name: &str) -> std::io::Result<String> {
    value.into_string().map_err(|_| invalid_input(format!("{field_name} must be valid UTF-8")))
}

fn invalid_input(message: impl Into<String>) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidInput, message.into())
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

fn non_zero(value: usize) -> NonZeroUsize {
    match NonZeroUsize::new(value) {
        Some(value) => value,
        None => unreachable!("buffer sizes must be non-zero"),
    }
}

impl Default for AllocatorConfig {
    fn default() -> Self {
        Self {
            read_buffer_size: READ_WRITE_MAX as usize,
            read_buffer_count: 2048,
            write_buffer_size: READ_WRITE_MAX as usize,
            write_buffer_count: 2048,
        }
    }
}

impl AllocatorConfig {
    fn from_file_config(config: Option<FileAllocatorConfig>) -> std::io::Result<Self> {
        let defaults = Self::default();
        let Some(config) = config else {
            return Ok(defaults);
        };

        Ok(Self {
            read_buffer_size: require_non_zero_usize(
                config.read_buffer_size.unwrap_or(defaults.read_buffer_size),
                "allocator.read_buffer_size",
            )?,
            read_buffer_count: require_non_zero_usize(
                config.read_buffer_count.unwrap_or(defaults.read_buffer_count),
                "allocator.read_buffer_count",
            )?,
            write_buffer_size: require_non_zero_usize(
                config.write_buffer_size.unwrap_or(defaults.write_buffer_size),
                "allocator.write_buffer_size",
            )?,
            write_buffer_count: require_non_zero_usize(
                config.write_buffer_count.unwrap_or(defaults.write_buffer_count),
                "allocator.write_buffer_count",
            )?,
        })
    }
}

fn require_non_zero_usize(value: usize, field_name: &str) -> std::io::Result<usize> {
    if value == 0 {
        Err(invalid_input(format!("{field_name} must be greater than zero")))
    } else {
        Ok(value)
    }
}
