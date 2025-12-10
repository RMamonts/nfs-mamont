use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::time::SystemTime;

use crate::vfs::file;

pub struct Export {
    pub registry: HashMap<file::Uid, FileNode>,
    pub next_uid: u64,
    pub root: PathBuf
}

// TODO(artemiipatov: attributes, file types, etc.)
pub struct FileNode {
    pub filepath: PathBuf,
    pub attrs: file::Attr,
}

impl file::Attr {
    pub fn new() -> file::Attr {
        file::Attr {
            file_type: file::Type::Regular,
            mode: file::Mode::Unchecked as u32,
            nlink: 0,
            uid: 0,
            gid: 0,
            size: 0,
            used: 0,
            device: None,
            fsid: 0,
            fileid:0,
            atime: file::Time { seconds: 0, nanos: 0 },
            mtime: file::Time { seconds: 0, nanos: 0 },
            ctime: file::Time { seconds: 0, nanos: 0 },
        }
    }
}

impl FileNode {
    pub fn new(path: PathBuf) -> FileNode {
        FileNode {
            filepath: path,
            attrs: file::Attr::new(),
        }
    }
}   

impl Export {
    pub fn make_export(dir: &str) -> io::Result<Export>  {
        let dir_content = fs::read_dir(dir)?;
    
        let mut export = Export {
            registry: HashMap::new(),
            next_uid: 0,
            root: dir.into()
        };
    
        for entry in dir_content {
            let entry = entry?;
            let file_node = FileNode::new(entry.path());
    
            export.registry.insert(file::Uid(export.next_uid.to_le_bytes()), file_node);
            export.next_uid = export.next_uid + 1;
        }
    
        Ok(export)
    }

    pub fn get_file_node(&self, uid: file::Uid) -> Option<&FileNode> {
        self.registry.get(&uid)
    }
}

