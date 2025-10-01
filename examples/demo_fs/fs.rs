use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;

use async_trait::async_trait;
use tokio::sync::RwLock;

use crate::fs_contents::FSContents;
use crate::fs_entry::{make_dir, make_file, FSEntry};
use nfs_mamont::vfs;
use nfs_mamont::xdr::nfs3;
use nfs_mamont::xdr::nfs3::dir::mknoddata3;
use nfs_mamont::xdr::nfs3::ftype3;

/// Demo implementation of an NFS file system.
/// Provides a simple in-memory file system that supports basic NFS operations.
#[derive(Debug)]
pub struct DemoFS {
    /// Map of all file system entries, protected by a tokio RwLock for concurrent access
    fs: RwLock<HashMap<nfs3::fileid3, FSEntry>>,
    /// File ID of the root directory
    rootdir: nfs3::fileid3,
    generation: u64,
}

impl Default for DemoFS {
    /// Creates a new DemoFS with just the root directory.
    ///
    /// Initializes an empty file system with only the special entry at id 0
    /// and the root directory at id 1.
    fn default() -> DemoFS {
        let mut map = HashMap::new();
        map.insert(0, make_file("", 0, 0, &[]));
        map.insert(1, make_dir("/", 1, 1, vec![]));
        let now = SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis();
        DemoFS { fs: RwLock::new(map), rootdir: 1, generation: now as u64 }
    }
}

// For this demo file system we let the handle just be the file
// there is only 1 file. a.txt.
/// Implementation of the NFSFileSystem trait for DemoFS.
/// Provides all required NFS operations for the demo file system.
#[async_trait]
impl vfs::NFSFileSystem for DemoFS {
    fn generation(&self) -> u64 {
        self.generation
    }

    /// Returns the file ID of the root directory.
    fn root_dir(&self) -> nfs3::fileid3 {
        self.rootdir
    }

    /// Returns the capabilities of this file system.
    /// This demo supports both read and write operations.
    fn capabilities(&self) -> vfs::Capabilities {
        vfs::Capabilities::ReadWrite
    }

    /// Writes data to a file at the specified offset.
    /// Resizes the file if needed and updates its attributes.
    async fn write(
        &self,
        id: nfs3::fileid3,
        offset: u64,
        data: &[u8],
    ) -> Result<nfs3::fattr3, nfs3::nfsstat3> {
        {
            let mut fs = self.fs.write().await;

            // Get file entry and verify it's a file
            let entry = fs.get_mut(&id).ok_or(nfs3::nfsstat3::NFS3ERR_NOENT)?;

            let shared_bytes = match &mut entry.contents {
                FSContents::File(bytes) => bytes,
                _ => return Err(nfs3::nfsstat3::NFS3ERR_IO),
            };

            let new_size = {
                // Write data to file
                let mut bytes = shared_bytes.write().await;
                let offset = offset as usize;

                // Resize if needed and copy data
                if offset + data.len() > bytes.len() {
                    bytes.resize(offset + data.len(), 0);
                }
                bytes[offset..offset + data.len()].copy_from_slice(data);

                bytes.len() as u64
            };

            // Update size for all entries sharing this file
            let shared_ptr = Arc::as_ptr(shared_bytes);
            for entry in fs.values_mut() {
                if let FSContents::File(b) = &entry.contents {
                    if Arc::as_ptr(b) == shared_ptr {
                        entry.attr.size = new_size;
                        entry.attr.used = new_size;
                    }
                }
            }
        }

        self.getattr(id).await
    }

    /// Creates a new file in the specified directory.
    /// Adds the new file to the parent directory's contents.
    async fn create(
        &self,
        dirid: nfs3::fileid3,
        filename: &nfs3::filename3,
        _attr: nfs3::sattr3,
    ) -> Result<(nfs3::fileid3, nfs3::fattr3), nfs3::nfsstat3> {
        let newid: nfs3::fileid3;
        {
            let mut fs = self.fs.write().await;
            newid = fs.len() as nfs3::fileid3;
            fs.insert(
                newid,
                make_file(std::str::from_utf8(filename).unwrap(), newid, dirid, "".as_bytes()),
            );
            if let Some(FSContents::Directory(dir)) = fs.get_mut(&dirid).map(|e| &mut e.contents) {
                dir.push(newid);
            }
        }
        Ok((newid, self.getattr(newid).await.unwrap()))
    }

    /// Creates a file exclusively (not supported in this demo).
    async fn create_exclusive(
        &self,
        _dirid: nfs3::fileid3,
        _filename: &nfs3::filename3,
    ) -> Result<nfs3::fileid3, nfs3::nfsstat3> {
        Err(nfs3::nfsstat3::NFS3ERR_NOTSUPP)
    }

    /// Looks up a file or directory by name within a directory.
    /// Handles special cases for '.' and '..' directory entries.
    async fn lookup(
        &self,
        dirid: nfs3::fileid3,
        filename: &nfs3::filename3,
    ) -> Result<nfs3::fileid3, nfs3::nfsstat3> {
        let fs = self.fs.read().await;
        let entry = fs.get(&dirid).ok_or(nfs3::nfsstat3::NFS3ERR_NOENT)?;
        if let FSContents::File(_) = entry.contents {
            return Err(nfs3::nfsstat3::NFS3ERR_NOTDIR);
        } else if let FSContents::Directory(dir) = &entry.contents {
            // if looking for dir/. its the current directory
            if filename[..] == [b'.'] {
                return Ok(dirid);
            }
            // if looking for dir/.. its the parent directory
            if filename[..] == [b'.', b'.'] {
                return Ok(entry.parent);
            }
            for i in dir {
                if let Some(f) = fs.get(i) {
                    if f.name[..] == filename[..] {
                        return Ok(*i);
                    }
                }
            }
        }
        Err(nfs3::nfsstat3::NFS3ERR_NOENT)
    }

    /// Gets the attributes of a file system entry.
    async fn getattr(&self, id: nfs3::fileid3) -> Result<nfs3::fattr3, nfs3::nfsstat3> {
        let fs = self.fs.read().await;
        let entry = fs.get(&id).ok_or(nfs3::nfsstat3::NFS3ERR_NOENT)?;
        Ok(entry.attr)
    }

    /// Sets attributes for a file system entry.
    /// Updates times, ownership, and file size as requested.
    async fn setattr(
        &self,
        id: nfs3::fileid3,
        setattr: nfs3::sattr3,
    ) -> Result<nfs3::fattr3, nfs3::nfsstat3> {
        let mut fs = self.fs.write().await;
        let entry = fs.get_mut(&id).ok_or(nfs3::nfsstat3::NFS3ERR_NOENT)?;
        match setattr.atime {
            nfs3::set_atime::DONT_CHANGE => {}
            nfs3::set_atime::SET_TO_CLIENT_TIME(c) => {
                entry.attr.atime = c;
            }
            nfs3::set_atime::SET_TO_SERVER_TIME => {
                let d = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap();
                entry.attr.atime.seconds = d.as_secs() as u32;
                entry.attr.atime.nseconds = d.subsec_nanos();
            }
        };
        match setattr.mtime {
            nfs3::set_mtime::DONT_CHANGE => {}
            nfs3::set_mtime::SET_TO_CLIENT_TIME(c) => {
                entry.attr.mtime = c;
            }
            nfs3::set_mtime::SET_TO_SERVER_TIME => {
                let d = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap();
                entry.attr.mtime.seconds = d.as_secs() as u32;
                entry.attr.mtime.nseconds = d.subsec_nanos();
            }
        };
        if let Some(u) = setattr.uid {
            entry.attr.uid = u
        }
        if let Some(u) = setattr.gid {
            entry.attr.gid = u
        }
        if let Some(s) = setattr.size {
            entry.attr.size = s;
            entry.attr.used = s;
            if let FSContents::File(shared_bytes) = &mut entry.contents {
                let mut bytes = shared_bytes.write().await;
                bytes.resize(s as usize, 0);
            }
        }
        Ok(entry.attr)
    }

    /// Reads data from a file at the specified offset.
    /// Returns the data and an EOF indicator.
    async fn read(
        &self,
        id: nfs3::fileid3,
        offset: u64,
        count: u32,
    ) -> Result<(Vec<u8>, bool), nfs3::nfsstat3> {
        let fs = self.fs.read().await;
        let entry = fs.get(&id).ok_or(nfs3::nfsstat3::NFS3ERR_NOENT)?;
        if let FSContents::Directory(_) = entry.contents {
            return Err(nfs3::nfsstat3::NFS3ERR_ISDIR);
        } else if let FSContents::File(shared_bytes) = &entry.contents {
            let bytes = shared_bytes.read().await;

            let mut start = offset as usize;
            let mut end = offset as usize + count as usize;
            let eof = end >= bytes.len();
            if start >= bytes.len() {
                start = bytes.len();
            }
            if end > bytes.len() {
                end = bytes.len();
            }
            return Ok((bytes[start..end].to_vec(), eof));
        }
        Err(nfs3::nfsstat3::NFS3ERR_NOENT)
    }

    /// Reads directory entries, starting after the specified entry ID.
    /// Returns a list of directory entries and an indicator if there are more entries.
    async fn readdir(
        &self,
        dirid: nfs3::fileid3,
        start_after: nfs3::fileid3,
        max_entries: usize,
    ) -> Result<vfs::ReadDirResult, nfs3::nfsstat3> {
        let fs = self.fs.read().await;
        let entry = fs.get(&dirid).ok_or(nfs3::nfsstat3::NFS3ERR_NOENT)?;
        if let FSContents::File(_) = entry.contents {
            return Err(nfs3::nfsstat3::NFS3ERR_NOTDIR);
        } else if let FSContents::Directory(dir) = &entry.contents {
            let mut ret = vfs::ReadDirResult { entries: Vec::new(), end: false };
            let mut start_index = 0;
            if start_after > 0 {
                if let Some(pos) = dir.iter().position(|&r| r == start_after) {
                    start_index = pos + 1;
                } else {
                    return Err(nfs3::nfsstat3::NFS3ERR_BAD_COOKIE);
                }
            }
            let remaining_length = dir.len() - start_index;

            for i in dir[start_index..].iter() {
                if let Some(f) = fs.get(i) {
                    ret.entries.push(vfs::DirEntry {
                        fileid: *i,
                        name: f.name.clone(),
                        attr: f.attr,
                    });
                }
                if ret.entries.len() >= max_entries {
                    break;
                }
            }
            if ret.entries.len() == remaining_length {
                ret.end = true;
            }
            return Ok(ret);
        }
        Err(nfs3::nfsstat3::NFS3ERR_NOENT)
    }

    /// Removes a file or empty directory from a directory.
    async fn remove(
        &self,
        dirid: nfs3::fileid3,
        filename: &nfs3::filename3,
    ) -> Result<(), nfs3::nfsstat3> {
        let mut fs = self.fs.write().await;
        let dir_entry = fs.get(&dirid).ok_or(nfs3::nfsstat3::NFS3ERR_NOENT)?;

        if let FSContents::File(_) = dir_entry.contents {
            return Err(nfs3::nfsstat3::NFS3ERR_NOTDIR);
        }

        // Find the file in the directory
        let id_to_remove = {
            if let FSContents::Directory(dir) = &fs.get(&dirid).unwrap().contents {
                let mut file_id = None;
                for &id in dir {
                    if let Some(file) = fs.get(&id) {
                        if file.name[..] == filename[..] {
                            file_id = Some(id);
                            break;
                        }
                    }
                }
                file_id.ok_or(nfs3::nfsstat3::NFS3ERR_NOENT)?
            } else {
                return Err(nfs3::nfsstat3::NFS3ERR_NOTDIR);
            }
        };

        // Type check
        let Some(target_entry) = fs.get(&id_to_remove) else {
            return Err(nfs3::nfsstat3::NFS3ERR_NOENT);
        };

        if let FSContents::Directory(contents) = &target_entry.contents {
            if !contents.is_empty() {
                return Err(nfs3::nfsstat3::NFS3ERR_NOTEMPTY);
            }
        }

        // Remove the file from the directory list
        if let FSContents::Directory(dir) = &mut fs.get_mut(&dirid).unwrap().contents {
            dir.retain(|&id| id != id_to_remove);
        }

        // Mark the file as deleted (in a real FS, we would completely remove it)
        // In our simple implementation, we just clear the name and contents
        if let Some(entry) = fs.get_mut(&id_to_remove) {
            entry.name = Vec::new().into();
            entry.contents = FSContents::File(Arc::new(RwLock::new(Vec::new())));
        }

        Ok(())
    }

    /// Renames a file or directory from one location to another.
    /// Handles various edge cases like moving between directories.
    async fn rename(
        &self,
        from_dirid: nfs3::fileid3,
        from_filename: &nfs3::filename3,
        to_dirid: nfs3::fileid3,
        to_filename: &nfs3::filename3,
    ) -> Result<(), nfs3::nfsstat3> {
        let mut fs = self.fs.write().await;

        // Find the file in the source directory
        let file_id = {
            if let FSContents::Directory(dir) = &fs.get(&from_dirid).unwrap().contents {
                let mut file_id = None;
                for &id in dir {
                    if let Some(file) = fs.get(&id) {
                        if file.name[..] == from_filename[..] {
                            file_id = Some(id);
                            break;
                        }
                    }
                }
                file_id.ok_or(nfs3::nfsstat3::NFS3ERR_NOENT)?
            } else {
                return Err(nfs3::nfsstat3::NFS3ERR_NOTDIR);
            }
        };

        // Check that the target directory exists
        if !fs
            .get(&to_dirid)
            .is_some_and(|entry| matches!(entry.contents, FSContents::Directory(_)))
        {
            return Err(nfs3::nfsstat3::NFS3ERR_NOTDIR);
        }

        // Find ID of the file to remove (if it exists)
        let to_remove_id = if let FSContents::Directory(dir) = &fs.get(&to_dirid).unwrap().contents
        {
            let mut to_remove = None;
            for &id in dir {
                if let Some(file) = fs.get(&id) {
                    if file.name[..] == to_filename[..] {
                        to_remove = Some(id);
                        break;
                    }
                }
            }
            to_remove
        } else {
            None
        };

        // If the file exists, remove it from the directory
        if let Some(id) = to_remove_id {
            if let FSContents::Directory(dir) = &mut fs.get_mut(&to_dirid).unwrap().contents {
                dir.retain(|&x| x != id);
            }
        }

        // If this is a move between directories
        if from_dirid != to_dirid {
            // Remove from the old directory
            if let FSContents::Directory(dir) = &mut fs.get_mut(&from_dirid).unwrap().contents {
                dir.retain(|&id| id != file_id);
            }

            // Add to the new directory
            if let FSContents::Directory(dir) = &mut fs.get_mut(&to_dirid).unwrap().contents {
                dir.push(file_id);
            }

            // Update the file's parent
            if let Some(entry) = fs.get_mut(&file_id) {
                entry.parent = to_dirid;
            }
        }

        // Update the file name
        if let Some(entry) = fs.get_mut(&file_id) {
            entry.name = to_filename.to_vec().into();
        }

        Ok(())
    }

    /// Creates a new directory with the specified name.
    async fn mkdir(
        &self,
        dirid: nfs3::fileid3,
        dirname: &nfs3::filename3,
    ) -> Result<(nfs3::fileid3, nfs3::fattr3), nfs3::nfsstat3> {
        let mut fs = self.fs.write().await;

        // Check that the parent directory exists
        if !fs.get(&dirid).is_some_and(|entry| matches!(entry.contents, FSContents::Directory(_))) {
            return Err(nfs3::nfsstat3::NFS3ERR_NOTDIR);
        }

        // Check that a directory with this name doesn't already exist
        if let FSContents::Directory(dir) = &fs.get(&dirid).unwrap().contents {
            if dir.iter().any(|&id| fs.get(&id).is_some_and(|file| file.name[..] == dirname[..])) {
                return Err(nfs3::nfsstat3::NFS3ERR_EXIST);
            }
        }

        // Create a new directory
        let newid = fs.len() as nfs3::fileid3;
        fs.insert(newid, make_dir(std::str::from_utf8(dirname).unwrap(), newid, dirid, Vec::new()));

        // Add the new directory to the parent
        if let FSContents::Directory(dir) = &mut fs.get_mut(&dirid).unwrap().contents {
            dir.push(newid);
        }

        // Update the parent directory's modification time
        let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap();
        if let Some(parent) = fs.get_mut(&dirid) {
            parent.attr.mtime.seconds = now.as_secs() as u32;
            parent.attr.mtime.nseconds = now.subsec_nanos();
        }

        // Return the ID and attributes of the new directory
        Ok((newid, fs.get(&newid).unwrap().attr))
    }

    /// Creates a symbolic link pointing to the specified path.
    async fn symlink(
        &self,
        dirid: nfs3::fileid3,
        linkname: &nfs3::filename3,
        symlink: &nfs3::nfspath3,
        _attr: &nfs3::sattr3,
    ) -> Result<(nfs3::fileid3, nfs3::fattr3), nfs3::nfsstat3> {
        // In our simple implementation, we'll just create a special file
        // with contents representing the path the link points to
        let mut fs = self.fs.write().await;

        // Check that the parent directory exists
        if !fs.get(&dirid).is_some_and(|entry| matches!(entry.contents, FSContents::Directory(_))) {
            return Err(nfs3::nfsstat3::NFS3ERR_NOTDIR);
        }

        // Create a new file but mark its type as a symbolic link
        let newid = fs.len() as nfs3::fileid3;
        let mut entry = make_file(std::str::from_utf8(linkname).unwrap(), newid, dirid, symlink);

        // Change type to symbolic link
        entry.attr.ftype = nfs3::ftype3::NF3LNK;

        fs.insert(newid, entry);

        // Add the new file to the parent directory
        if let FSContents::Directory(dir) = &mut fs.get_mut(&dirid).unwrap().contents {
            dir.push(newid);
        }

        // Update the parent directory's modification time
        let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap();
        if let Some(parent) = fs.get_mut(&dirid) {
            parent.attr.mtime.seconds = now.as_secs() as u32;
            parent.attr.mtime.nseconds = now.subsec_nanos();
        }

        // Return the ID and attributes of the new file
        Ok((newid, fs.get(&newid).unwrap().attr))
    }

    /// Reads the target of a symbolic link.
    async fn readlink(&self, id: nfs3::fileid3) -> Result<nfs3::nfspath3, nfs3::nfsstat3> {
        let fs = self.fs.read().await;

        // Check that the file exists
        let entry = fs.get(&id).ok_or(nfs3::nfsstat3::NFS3ERR_NOENT)?;

        // Use matching instead of comparing with ftype3::NF3LNK
        match entry.attr.ftype {
            nfs3::ftype3::NF3LNK => {
                // Get the symbolic link content
                if let FSContents::File(shared_bytes) = &entry.contents {
                    let bytes = shared_bytes.read().await;
                    // Convert Vec<u8> to nfspath3
                    return Ok(bytes.to_vec().into());
                }
                Err(nfs3::nfsstat3::NFS3ERR_INVAL)
            }
            _ => Err(nfs3::nfsstat3::NFS3ERR_INVAL),
        }
    }

    /// Creates a hard link to an existing file.
    async fn link(
        &self,
        file_id: nfs3::fileid3,
        target_dir_id: nfs3::fileid3,
        link_name: &nfs3::filename3,
    ) -> Result<nfs3::fattr3, nfs3::nfsstat3> {
        let mut fs = self.fs.write().await;

        // Check that the source file exists
        let source_file = fs.get(&file_id).ok_or(nfs3::nfsstat3::NFS3ERR_NOENT)?;

        // Check that the source is a file, not a directory
        if let FSContents::Directory(_) = source_file.contents {
            return Err(nfs3::nfsstat3::NFS3ERR_ISDIR);
        }

        // Check that the target directory exists
        if !fs
            .get(&target_dir_id)
            .is_some_and(|entry| matches!(entry.contents, FSContents::Directory(_)))
        {
            return Err(nfs3::nfsstat3::NFS3ERR_NOTDIR);
        }

        // Check if a file with the same name already exists in the target directory
        if let FSContents::Directory(dir) = &fs.get(&target_dir_id).unwrap().contents {
            if dir.iter().any(|&id| fs.get(&id).is_some_and(|file| file.name[..] == link_name[..]))
            {
                return Err(nfs3::nfsstat3::NFS3ERR_EXIST);
            }
        }

        // Create a new entry for the hard link
        let newid = fs.len() as nfs3::fileid3;

        let new_entry = FSEntry {
            id: newid,
            name: link_name.to_vec().into(),
            parent: target_dir_id,
            attr: source_file.attr,
            contents: source_file.contents.clone(),
        };

        // Add the new entry to the filesystem
        fs.insert(newid, new_entry);

        // Add the new entry to the target directory
        if let FSContents::Directory(dir) = &mut fs.get_mut(&target_dir_id).unwrap().contents {
            dir.push(newid);
        }

        // Update the link count of the original file
        if let Some(entry) = fs.get_mut(&file_id) {
            entry.attr.nlink += 1;
        }

        // Return the attributes of the new link
        Ok(fs.get(&newid).unwrap().attr)
    }

    /// Creates a special device node file.
    async fn mknod(
        &self,
        dir_id: nfs3::fileid3,
        diropr: nfs3::diropargs3,
        ftype: nfs3::dir::mknoddata3,
        attrs: &nfs3::sattr3,
    ) -> Result<(nfs3::fileid3, nfs3::fattr3), nfs3::nfsstat3> {
        let mut fs = self.fs.write().await;

        // Check that the parent directory exists
        if !fs.get(&dir_id).is_some_and(|entry| matches!(entry.contents, FSContents::Directory(_)))
        {
            return Err(nfs3::nfsstat3::NFS3ERR_NOTDIR);
        }

        // Check if a file with the same name already exists
        if let FSContents::Directory(dir) = &fs.get(&dir_id).unwrap().contents {
            if dir
                .iter()
                .any(|&id| fs.get(&id).is_some_and(|file| file.name[..] == diropr.name[..]))
            {
                return Err(nfs3::nfsstat3::NFS3ERR_EXIST);
            }
        }

        // Create a new entry based on the type
        let newid = fs.len() as nfs3::fileid3;
        let mut entry;
        match ftype {
            mknoddata3::NF3CHR(dev) => {
                entry = make_file(
                    str::from_utf8(diropr.name.0.as_slice()).unwrap(),
                    newid,
                    dir_id,
                    &[],
                );
                entry.attr.ftype = ftype3::NF3CHR;
                entry.attr.rdev = dev.device;
            }
            mknoddata3::NF3BLK(dev) => {
                entry = make_file(
                    str::from_utf8(diropr.name.0.as_slice()).unwrap(),
                    newid,
                    dir_id,
                    &[],
                );
                entry.attr.ftype = ftype3::NF3CHR;
                entry.attr.rdev = dev.device;
            }
            mknoddata3::NF3SOCK(_) => {
                entry = make_file(
                    str::from_utf8(diropr.name.0.as_slice()).unwrap(),
                    newid,
                    dir_id,
                    &[],
                );
                entry.attr.ftype = ftype3::NF3SOCK;
            }
            mknoddata3::NF3FIFO(_) => {
                entry = make_file(
                    str::from_utf8(diropr.name.0.as_slice()).unwrap(),
                    newid,
                    dir_id,
                    &[],
                );
                entry.attr.ftype = ftype3::NF3FIFO;
            }
            _ => {
                return Err(nfs3::nfsstat3::NFS3ERR_NOTSUPP);
            }
        }

        // Apply any additional attributes
        if let Some(mode) = attrs.mode {
            entry.attr.mode = mode;
        }

        if let Some(uid) = attrs.uid {
            entry.attr.uid = uid;
        }

        if let Some(gid) = attrs.gid {
            entry.attr.gid = gid;
        }

        // Add the new entry to the filesystem
        fs.insert(newid, entry);

        // Add the new entry to the parent directory
        if let FSContents::Directory(dir) = &mut fs.get_mut(&dir_id).unwrap().contents {
            dir.push(newid);
        }

        // Update the parent directory's modification time
        let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap();
        if let Some(parent) = fs.get_mut(&dir_id) {
            parent.attr.mtime.seconds = now.as_secs() as u32;
            parent.attr.mtime.nseconds = now.subsec_nanos();
        }

        // Return the ID and attributes of the new entry
        Ok((newid, fs.get(&newid).unwrap().attr))
    }

    /// Commits any pending writes to stable storage.
    /// In this in-memory implementation, it simply returns the current attributes.
    async fn commit(
        &self,
        id: nfs3::fileid3,
        _offset: u64,
        _count: u32,
    ) -> Result<nfs3::fattr3, nfs3::nfsstat3> {
        // In a real filesystem, this would ensure that the data written
        // to the file is committed to persistent storage.
        // For this demo, we'll just update the file's modification time
        // and return the attributes.

        let mut fs = self.fs.write().await;
        let entry = fs.get_mut(&id).ok_or(nfs3::nfsstat3::NFS3ERR_NOENT)?;

        // Update the file's modification time
        let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap();
        entry.attr.mtime.seconds = now.as_secs() as u32;
        entry.attr.mtime.nseconds = now.subsec_nanos();

        // Return the updated attributes
        Ok(entry.attr)
    }
}
