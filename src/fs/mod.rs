mod dispatcher;

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
    async fn create_exclusive_file(&self, parent: file::Uid, name: String) -> io::Result<File> {
        let path = self.export.root.join(name);
        let mut options: OpenOptions = OpenOptions::new();
        options.write(true).create(true);
        options.open(path).await
    }
}

impl Vfs for MamontFs {    
    async fn get_attr(&self, file: file::Uid) -> Result<Response>  {
        let file_node = self.export.get_file_node(uid)?;

        
    }

    async fn set_attr(&self, file: file::Uid, attr: SetAttr, guard: Option<SetAttrGuard>) -> Result<Response>  { !unimplemented!() }

    async fn lookup(&self, parent: file::Uid, name: String) -> Result<Response>  { !unimplemented!() }

    async fn access(&self, file: file::Uid, mask: AccessMask) -> Result<Response>  { !unimplemented!() }

    async fn read_link(&self, file: file::Uid) -> Result<Response>  { !unimplemented!() }

    async fn read(&self, file: file::Uid, offset: u64, count: u32) -> Result<Response>  { !unimplemented!() }

    async fn write(&self, file: file::Uid, offset: u64, data: Vec<u8>, mode: WriteMode) -> Result<Response>  { !unimplemented!() }

    async fn create(
        &mut self,
        parent: file::Uid,
        name: String,
        mode: CreateMode
    ) -> Result<Response> {
        /* Create file description */
        let file_uid = file::Uid(u64::to_be_bytes(self.export.next_uid));
        let file_node = FileNode { filepath: name.clone().into() };

        /* Create actual node */
        self.create_exclusive_file(parent, name).await; // TODO support different modes of creation

        /* Insert to export */
        self.export.registry.insert(file_uid, file_node);
        self.export.next_uid += 1;

        Ok(Response::Create(CreateResult {
            file: Some(file_uid),
            file_attr: Some(Attr { file_type: todo!(), mode: todo!(), nlink: todo!(), uid: todo!(), gid: todo!(), size: todo!(), used: todo!(), device: todo!(), fsid: todo!(), fileid: todo!(), atime: todo!(), mtime: todo!(), ctime: todo!() }),
            dir_wcc: WccData { before: None, after: None }
        }))
    }

    async fn make_dir(&self, parent: file::Uid, name: String, attr: SetAttr) -> Result<Response>  { !unimplemented!() }

    async fn make_symlink(&self, parent: file::Uid, name: String, target: &Path, attr: SetAttr) -> Result<Response>  { !unimplemented!() }

    async fn make_node(&self, parent: file::Uid, name: String, node: SpecialNode) -> Result<Response>  { !unimplemented!() }

    async fn remove(&self, parent: file::Uid, name: String) -> Result<Response>  { !unimplemented!() }

    async fn remove_dir(&self, parent: file::Uid, name: String) -> Result<Response>  { !unimplemented!() }

    async fn rename(
        &self,
        from_parent: file::Uid,
        from_name: String,
        to_parent: file::Uid,
        to_name: String,
    ) -> Result<Response>  { !unimplemented!() }

    async fn link(&self, source: file::Uid, new_parent: file::Uid, new_name: String) -> Result<Response>  { !unimplemented!() }

    async fn read_dir(
        &self,
        file: file::Uid,
        cookie: DirectoryCookie,
        verifier: CookieVerifier,
        max_bytes: u32,
    ) -> Result<Response>  { !unimplemented!() }

    async fn read_dir_plus(
        &self,
        file: file::Uid,
        cookie: DirectoryCookie,
        verifier: CookieVerifier,
        max_bytes: u32,
        max_files: u32,
    ) -> Result<Response>  { !unimplemented!() }

    async fn fs_stat(&self, file: file::Uid) -> Result<Response>  { !unimplemented!() }

    async fn fs_info(&self, file: file::Uid) -> Result<Response>  { !unimplemented!() }

    async fn path_conf(&self, file: file::Uid) -> Result<Response>  { !unimplemented!() }

    async fn commit(&self, file: file::Uid, offset: u64, count: u32) -> Result<Response>  { !unimplemented!() }
}