use std::net::SocketAddr;
use std::num::NonZeroUsize;
use std::path::Component;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::fs::READ_WRITE_MAX;

const DEFAULT_BIND: SocketAddr = SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED), 2049);
const DEFAULT_VFS_POOL_SIZE: usize = 10;

#[derive(Debug)]
pub struct RuntimeConfig {
    pub bind: SocketAddr,
    pub allocator: AllocatorConfig,
    pub vfs_pool_size: NonZeroUsize,
    pub exports: Vec<ConfiguredExport>,
}

#[derive(Debug)]
pub struct AllocatorConfig {
    pub read_buffer_size: NonZeroUsize,
    pub read_buffer_count: NonZeroUsize,
    pub write_buffer_size: NonZeroUsize,
    pub write_buffer_count: NonZeroUsize,
}

#[derive(Debug)]
pub struct ConfiguredExport {
    pub local_path: PathBuf,
    pub mount_path: String,
}

#[derive(Deserialize)]
struct FileConfig {
    listen: Option<FileListenConfig>,
    allocator: Option<FileAllocatorConfig>,
    server: Option<FileServerConfig>,
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
struct FileServerConfig {
    vfs_pool_size: Option<usize>,
}

#[derive(Deserialize)]
struct FileExportsConfig {
    root: PathBuf,
    paths: Vec<PathBuf>,
}

pub fn parse_runtime_config(
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
        Some(value) => parse_socket_addr(&os_string_into_string(value, "bind address")?)?,
        None => DEFAULT_BIND,
    };
    if args.next().is_some() {
        return Err(invalid_input("unexpected extra arguments"));
    }

    let export_root = resolve_export_root(Path::new(&first_arg))?;
    Ok(RuntimeConfig {
        bind,
        allocator: AllocatorConfig::default(),
        vfs_pool_size: NonZeroUsize::new(DEFAULT_VFS_POOL_SIZE).unwrap(),
        exports: vec![ConfiguredExport {
            local_path: export_root.clone(),
            mount_path: export_root.to_string_lossy().into_owned(),
        }],
    })
}

pub(crate) fn load_runtime_config(path: &Path) -> std::io::Result<RuntimeConfig> {
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

    let vfs_pool_size = non_zero(
        config.server.and_then(|s| s.vfs_pool_size).unwrap_or(DEFAULT_VFS_POOL_SIZE),
        "server.vfs_pool_size",
    )?;

    let bind = match config.listen.and_then(|listen| listen.addr) {
        Some(addr) => parse_socket_addr(&addr)?,
        None => DEFAULT_BIND,
    };

    Ok(RuntimeConfig {
        bind,
        allocator: AllocatorConfig::from_file_config(config.allocator)?,
        vfs_pool_size,
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

fn parse_socket_addr(s: &str) -> std::io::Result<SocketAddr> {
    s.parse::<SocketAddr>()
        .map_err(|e| invalid_input(format!("invalid bind address \"{s}\": {e}")))
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

fn non_zero(value: usize, field_name: &str) -> std::io::Result<NonZeroUsize> {
    NonZeroUsize::new(value)
        .ok_or_else(|| invalid_input(format!("{field_name} must be greater than zero")))
}

impl Default for AllocatorConfig {
    fn default() -> Self {
        Self {
            read_buffer_size: NonZeroUsize::new(READ_WRITE_MAX as usize).unwrap(),
            read_buffer_count: NonZeroUsize::new(2048).unwrap(),
            write_buffer_size: NonZeroUsize::new(READ_WRITE_MAX as usize).unwrap(),
            write_buffer_count: NonZeroUsize::new(2048).unwrap(),
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
            read_buffer_size: non_zero(
                config.read_buffer_size.unwrap_or(defaults.read_buffer_size.get()),
                "allocator.read_buffer_size",
            )?,
            read_buffer_count: non_zero(
                config.read_buffer_count.unwrap_or(defaults.read_buffer_count.get()),
                "allocator.read_buffer_count",
            )?,
            write_buffer_size: non_zero(
                config.write_buffer_size.unwrap_or(defaults.write_buffer_size.get()),
                "allocator.write_buffer_size",
            )?,
            write_buffer_count: non_zero(
                config.write_buffer_count.unwrap_or(defaults.write_buffer_count.get()),
                "allocator.write_buffer_count",
            )?,
        })
    }
}
