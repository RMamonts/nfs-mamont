use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::PathBuf;

use crate::vfs::file;

pub struct Export {
    pub registry: HashMap<file::Uid, FileNode>,
    pub next_uid: u64,
    pub root: PathBuf
}

// TODO(artemiipatov: attributes, file types, etc.)
pub struct FileNode {
    pub filepath: PathBuf
}

pub fn make_export(dir: &str) -> io::Result<Export>  {
    let dir_content = fs::read_dir(dir)?;

    let mut export = Export {
        registry: HashMap::new(),
        next_uid: 0,
        root: dir.into()
    };

    for entry in dir_content {
        let entry = entry?;
        let file_node = FileNode { filepath: entry.path() };

        export.registry.insert(file::Uid(export.next_uid.to_le_bytes()), file_node);
        export.next_uid = export.next_uid + 1;
    }

    Ok(export)
}