use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;

use crate::mount::{ExportEntry, MountEntry};
use crate::vfs::file;

mod dump;
mod export;
mod mnt;
mod umnt;
mod umntall;

#[allow(dead_code)]
#[derive(Default)]
struct ExportRegistry {
    // one dir can have only one export
    #[allow(dead_code)]
    by_directory: HashMap<file::Path, ExportEntry>,
}

#[allow(dead_code)]
#[derive(Default)]
struct MountRegistry {
    // one client can mount multiple dirs
    #[allow(dead_code)]
    by_client: HashMap<SocketAddr, HashSet<MountEntry>>,
}

#[allow(dead_code)]
struct Service {
    // what's available to mount
    #[allow(dead_code)]
    exports: ExportRegistry,
    // who has mounted what
    #[allow(dead_code)]
    mounts: MountRegistry,
}
