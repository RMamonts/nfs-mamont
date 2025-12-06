use crate::export::FileNode;
use crate::vfs::file::Attr;
use crate::vfs::*;
use crate::export;

use std::io;
use std::path::{Path, PathBuf};

use tokio::fs::{self, File, OpenOptions};

pub struct MamontFs {
    export: export::Export
}

impl MamontFs {
    async fn create_exclusive_file(&self, parent: &file::Uid, name: &str) -> io::Result<File> {
        let path = self.export.root.join(name);
        let mut options: OpenOptions = OpenOptions::new();
        options.write(true).create(true);
        options.open(path).await
    }
}

impl Vfs for MamontFs {
    async fn create(
        &mut self,
        parent: &file::Uid,
        name: &str,
        mode: CreateMode
    ) -> Result<CreatedNode> {

        /* Create file description */
        let file_uid = file::Uid(u64::to_be_bytes(self.export.next_uid));
        let file_node = FileNode { filepath: name.into() };

        /* Create actual node */
        self.create_exclusive_file(parent, name).await; // TODO support different modes of creation

        /* Insert to export */
        self.export.registry.insert(file_uid, file_node);
        self.export.next_uid += 1;

        Ok(CreatedNode {
            file: file_uid,
            attr: Attr { file_type: todo!(), mode: todo!(), nlink: todo!(), uid: todo!(), gid: todo!(), size: todo!(), used: todo!(), device: todo!(), fsid: todo!(), fileid: todo!(), atime: todo!(), mtime: todo!(), ctime: todo!() },
            directory_wcc: WccData { before: None, after: None }
        })
    }
}