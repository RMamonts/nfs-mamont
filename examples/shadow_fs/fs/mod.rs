use std::cmp;
use std::ffi::{OsStr, OsString};
use std::io;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;

use tokio::fs::{self, File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::task;

use nfs_mamont::vfs;

mod shadow;
mod state;
mod utils;

pub use shadow::ShadowFS;

use utils::{
    apply_setattr, ensure_supported_attr, file_name_string, join_child, map_io_error,
    ENTRY_ESTIMATE_BYTES,
};

#[async_trait::async_trait]
impl vfs::Vfs for ShadowFS {
    async fn get_attr(&self, handle: &vfs::FileHandle) -> vfs::VfsResult<vfs::FileAttr> {
        let id = Self::decode_handle(handle)?;
        let meta = self.metadata_for_id(id).await?;
        Ok(Self::attr_from_meta(id, &meta))
    }

    async fn set_attr(
        &self,
        handle: &vfs::FileHandle,
        attr: vfs::SetAttr,
        guard: vfs::SetAttrGuard,
    ) -> vfs::VfsResult<vfs::WccData> {
        let id = Self::decode_handle(handle)?;
        let rel = self.rel_path_from_id(id).await?;
        let abs = self.full_path(&rel);
        let before_meta = self.metadata_for_rel(&rel).await?;
        let before_attr = Self::attr_from_meta(id, &before_meta);

        if let vfs::SetAttrGuard::Check { ctime } = guard {
            if before_attr.ctime != ctime {
                return Err(vfs::NfsError::NotSync);
            }
        }

        apply_setattr(&abs, &attr).await?;

        let after_meta = self.metadata_for_rel(&rel).await?;
        let after_attr = Self::attr_from_meta(id, &after_meta);
        Ok(vfs::WccData {
            before: Some(Self::digest_from_attr(&before_attr)),
            after: Some(after_attr),
        })
    }

    async fn lookup(
        &self,
        parent: &vfs::FileHandle,
        name: &vfs::FileName,
    ) -> vfs::VfsResult<vfs::LookupResult> {
        let parent_id = Self::decode_handle(parent)?;
        let parent_rel = self.rel_path_from_id(parent_id).await?;
        let child_name = OsString::from(&name.0);
        let child_rel = join_child(&parent_rel, &child_name)?;
        let child_abs = self.full_path(&child_rel);
        let meta = fs::symlink_metadata(&child_abs).await.map_err(map_io_error)?;
        let fileid = self.ensure_entry(child_rel.clone()).await;
        let object_attr = Self::attr_from_meta(fileid, &meta);
        let dir_meta = self.metadata_for_rel(&parent_rel).await?;
        let directory_attr = Some(Self::attr_from_meta(parent_id, &dir_meta));
        Ok(vfs::LookupResult { handle: Self::encode_handle(fileid), object_attr, directory_attr })
    }

    async fn access(
        &self,
        handle: &vfs::FileHandle,
        mask: vfs::AccessMask,
    ) -> vfs::VfsResult<vfs::AccessResult> {
        let id = Self::decode_handle(handle)?;
        let rel = self.rel_path_from_id(id).await?;
        let abs = self.full_path(&rel);
        let meta = fs::symlink_metadata(&abs).await.map_err(map_io_error)?;

        let mut granted = vfs::AccessMask::empty();
        let file_type = meta.file_type();
        let mode = meta.mode();

        // TODO: make more precision checks based on user/group ownership
        let has_read = (mode & 0o444) != 0;
        let has_write = (mode & 0o222) != 0;
        let has_exec = (mode & 0o111) != 0;

        if mask.contains(vfs::AccessMask::READ) && (has_read || file_type.is_dir()) {
            granted.insert(vfs::AccessMask::READ);
        }

        if mask.contains(vfs::AccessMask::LOOKUP) && file_type.is_dir() && has_exec {
            granted.insert(vfs::AccessMask::LOOKUP);
        }

        if mask.contains(vfs::AccessMask::MODIFY) && has_write {
            granted.insert(vfs::AccessMask::MODIFY);
        }

        if mask.contains(vfs::AccessMask::EXTEND) && has_write {
            granted.insert(vfs::AccessMask::EXTEND);
        }

        if mask.contains(vfs::AccessMask::EXECUTE) && has_exec {
            granted.insert(vfs::AccessMask::EXECUTE);
        }

        if mask.contains(vfs::AccessMask::DELETE) {
            let parent_rel = rel.parent().map(Path::to_path_buf).unwrap_or_default();
            let parent_abs = self.full_path(&parent_rel);
            if let Ok(parent_meta) = fs::symlink_metadata(&parent_abs).await {
                if (parent_meta.mode() & 0o300) != 0 {
                    granted.insert(vfs::AccessMask::DELETE);
                }
            }
        }

        let attr = Some(Self::attr_from_meta(id, &meta));
        Ok(vfs::AccessResult { granted, file_attr: attr })
    }

    async fn read_link(
        &self,
        handle: &vfs::FileHandle,
    ) -> vfs::VfsResult<(vfs::SymlinkTarget, Option<vfs::FileAttr>)> {
        let id = Self::decode_handle(handle)?;
        let rel = self.rel_path_from_id(id).await?;
        let abs = self.full_path(&rel);
        let target = fs::read_link(&abs).await.map_err(map_io_error)?;
        let attr =
            self.metadata_for_rel(&rel).await.ok().map(|meta| Self::attr_from_meta(id, &meta));
        Ok((vfs::SymlinkTarget(target.to_string_lossy().into_owned()), attr))
    }

    async fn read(
        &self,
        handle: &vfs::FileHandle,
        offset: u64,
        count: u32,
    ) -> vfs::VfsResult<vfs::ReadResult> {
        let id = Self::decode_handle(handle)?;
        let rel = self.rel_path_from_id(id).await?;
        let abs = self.full_path(&rel);
        let meta = fs::metadata(&abs).await.map_err(map_io_error)?;
        let size = meta.len();
        let to_read =
            if offset >= size { 0 } else { cmp::min(count as u64, size - offset) as usize };

        let mut data = vec![0u8; to_read];
        if to_read > 0 {
            let mut file = File::open(&abs).await.map_err(map_io_error)?;
            file.seek(io::SeekFrom::Start(offset)).await.map_err(map_io_error)?;
            file.read_exact(&mut data).await.map_err(map_io_error)?;
        }

        let attr = Self::attr_from_meta(id, &meta);
        Ok(vfs::ReadResult { data, file_attr: Some(attr) })
    }

    async fn write(
        &self,
        handle: &vfs::FileHandle,
        offset: u64,
        data: &[u8],
        mode: vfs::WriteMode,
    ) -> vfs::VfsResult<vfs::WriteResult> {
        let id = Self::decode_handle(handle)?;
        let rel = self.rel_path_from_id(id).await?;
        let abs = self.full_path(&rel);
        let mut file = OpenOptions::new().write(true).open(&abs).await.map_err(map_io_error)?;
        file.seek(io::SeekFrom::Start(offset)).await.map_err(map_io_error)?;
        file.write_all(data).await.map_err(map_io_error)?;
        file.flush().await.map_err(map_io_error)?;
        match mode {
            vfs::WriteMode::Unstable => {}
            vfs::WriteMode::DataSync => {
                file.sync_data().await.map_err(map_io_error)?;
            }
            vfs::WriteMode::FileSync => {
                file.sync_all().await.map_err(map_io_error)?;
            }
        }

        let meta = fs::metadata(&abs).await.map_err(map_io_error)?;
        let attr = Self::attr_from_meta(id, &meta);
        Ok(vfs::WriteResult {
            count: data.len() as u32,
            committed: mode,
            verifier: self.stable_verifier(),
            file_attr: Some(attr),
        })
    }

    async fn create(
        &self,
        parent: &vfs::FileHandle,
        name: &vfs::FileName,
        mode: vfs::CreateMode,
    ) -> vfs::VfsResult<vfs::CreatedNode> {
        let parent_rel = self.rel_path_from_handle(parent).await?;
        let name_os = OsString::from(&name.0);
        let child_rel = join_child(&parent_rel, &name_os)?;
        let child_abs = self.full_path(&child_rel);

        let exists = fs::symlink_metadata(&child_abs).await.is_ok();

        match mode {
            vfs::CreateMode::Exclusive { .. } | vfs::CreateMode::Guarded { .. } if exists => {
                return Err(vfs::NfsError::Exist);
            }
            _ => {}
        }

        let mut options = OpenOptions::new();
        options.write(true).create(true);
        if matches!(mode, vfs::CreateMode::Exclusive { .. }) {
            options.create_new(true);
        }
        options.open(&child_abs).await.map_err(map_io_error)?;

        if let vfs::CreateMode::Unchecked { attr } | vfs::CreateMode::Guarded { attr, .. } = mode {
            let _ = apply_setattr(&child_abs, &attr).await;
        }

        let meta = fs::symlink_metadata(&child_abs).await.map_err(map_io_error)?;
        let fileid = self.ensure_entry(child_rel).await;
        Ok(vfs::CreatedNode {
            handle: Self::encode_handle(fileid),
            attr: Self::attr_from_meta(fileid, &meta),
            directory_wcc: vfs::WccData { before: None, after: None },
        })
    }

    async fn make_dir(
        &self,
        parent: &vfs::FileHandle,
        name: &vfs::FileName,
        attr: vfs::SetAttr,
    ) -> vfs::VfsResult<vfs::CreatedNode> {
        let parent_rel = self.rel_path_from_handle(parent).await?;
        let name_os = OsString::from(&name.0);
        let child_rel = join_child(&parent_rel, &name_os)?;
        let child_abs = self.full_path(&child_rel);
        fs::create_dir(&child_abs).await.map_err(map_io_error)?;
        ensure_supported_attr(&attr, false, true)?;
        if let Some(mode) = attr.mode {
            let path = child_abs.clone();
            task::spawn_blocking(move || {
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode))
            })
            .await
            .map_err(|_| vfs::NfsError::ServerFault)?
            .map_err(map_io_error)?;
        }
        let meta = fs::symlink_metadata(&child_abs).await.map_err(map_io_error)?;
        let fileid = self.ensure_entry(child_rel).await;
        Ok(vfs::CreatedNode {
            handle: Self::encode_handle(fileid),
            attr: Self::attr_from_meta(fileid, &meta),
            directory_wcc: vfs::WccData { before: None, after: None },
        })
    }

    async fn make_symlink(
        &self,
        parent: &vfs::FileHandle,
        name: &vfs::FileName,
        target: &vfs::SymlinkTarget,
        attr: vfs::SetAttr,
    ) -> vfs::VfsResult<vfs::CreatedNode> {
        ensure_supported_attr(&attr, false, false)?;
        let parent_rel = self.rel_path_from_handle(parent).await?;
        let name_os = OsString::from(&name.0);
        let child_rel = join_child(&parent_rel, &name_os)?;
        let child_abs = self.full_path(&child_rel);
        use std::os::unix::fs as unix_fs;
        let target_path = target.0.clone();
        let path_clone = child_abs.clone();
        task::spawn_blocking(move || unix_fs::symlink(&target_path, &path_clone))
            .await
            .map_err(|_| vfs::NfsError::ServerFault)?
            .map_err(map_io_error)?;

        let meta = fs::symlink_metadata(&child_abs).await.map_err(map_io_error)?;
        let fileid = self.ensure_entry(child_rel).await;
        Ok(vfs::CreatedNode {
            handle: Self::encode_handle(fileid),
            attr: Self::attr_from_meta(fileid, &meta),
            directory_wcc: vfs::WccData { before: None, after: None },
        })
    }

    // TODO: Support creating special nodes (block, char, fifo, socket) when needed.
    async fn make_node(
        &self,
        _parent: &vfs::FileHandle,
        _name: &vfs::FileName,
        _node: vfs::SpecialNode,
    ) -> vfs::VfsResult<vfs::CreatedNode> {
        Err(vfs::NfsError::NotSupp)
    }

    async fn remove(
        &self,
        parent: &vfs::FileHandle,
        name: &vfs::FileName,
    ) -> vfs::VfsResult<vfs::RemovalResult> {
        let parent_rel = self.rel_path_from_handle(parent).await?;
        let name_os = OsString::from(&name.0);
        let child_rel = join_child(&parent_rel, &name_os)?;
        let child_abs = self.full_path(&child_rel);
        let meta = fs::symlink_metadata(&child_abs).await.map_err(map_io_error)?;
        if meta.is_dir() {
            return Err(vfs::NfsError::IsDir);
        }
        fs::remove_file(&child_abs).await.map_err(map_io_error)?;
        self.remove_entry(&child_rel).await;
        Ok(vfs::RemovalResult { directory_wcc: vfs::WccData { before: None, after: None } })
    }

    async fn remove_dir(
        &self,
        parent: &vfs::FileHandle,
        name: &vfs::FileName,
    ) -> vfs::VfsResult<vfs::RemovalResult> {
        let parent_rel = self.rel_path_from_handle(parent).await?;
        let name_os = OsString::from(&name.0);
        let child_rel = join_child(&parent_rel, &name_os)?;
        let child_abs = self.full_path(&child_rel);
        fs::remove_dir(&child_abs).await.map_err(map_io_error)?;
        self.remove_entry(&child_rel).await;
        Ok(vfs::RemovalResult { directory_wcc: vfs::WccData { before: None, after: None } })
    }

    async fn rename(
        &self,
        from_parent: &vfs::FileHandle,
        from_name: &vfs::FileName,
        to_parent: &vfs::FileHandle,
        to_name: &vfs::FileName,
    ) -> vfs::VfsResult<vfs::RenameResult> {
        let from_parent_rel = self.rel_path_from_handle(from_parent).await?;
        let to_parent_rel = self.rel_path_from_handle(to_parent).await?;

        let from_rel = join_child(&from_parent_rel, OsStr::new(&from_name.0))?;
        if from_rel.as_os_str().is_empty() {
            return Err(vfs::NfsError::Perm);
        }
        let to_rel = join_child(&to_parent_rel, OsStr::new(&to_name.0))?;
        let from_abs = self.full_path(&from_rel);
        let to_abs = self.full_path(&to_rel);

        let source_id = self.ensure_entry(from_rel.clone()).await;

        fs::rename(&from_abs, &to_abs).await.map_err(map_io_error)?;

        self.rename_entry(source_id, to_rel.clone()).await;

        Ok(vfs::RenameResult {
            from_directory_wcc: vfs::WccData { before: None, after: None },
            to_directory_wcc: vfs::WccData { before: None, after: None },
        })
    }

    async fn link(
        &self,
        source: &vfs::FileHandle,
        new_parent: &vfs::FileHandle,
        new_name: &vfs::FileName,
    ) -> vfs::VfsResult<vfs::LinkResult> {
        let src_rel = self.rel_path_from_handle(source).await?;
        let dst_parent_rel = self.rel_path_from_handle(new_parent).await?;
        let dst_rel = join_child(&dst_parent_rel, OsStr::new(&new_name.0))?;
        let src_abs = self.full_path(&src_rel);
        let dst_abs = self.full_path(&dst_rel);
        fs::hard_link(&src_abs, &dst_abs).await.map_err(map_io_error)?;
        let fileid = self.ensure_entry(dst_rel).await;
        let meta = fs::metadata(&dst_abs).await.map_err(map_io_error)?;
        Ok(vfs::LinkResult {
            new_file_attr: Some(Self::attr_from_meta(fileid, &meta)),
            directory_wcc: vfs::WccData { before: None, after: None },
        })
    }

    async fn read_dir(
        &self,
        handle: &vfs::FileHandle,
        cookie: vfs::DirectoryCookie,
        verifier: vfs::CookieVerifier,
        max_bytes: u32,
    ) -> vfs::VfsResult<vfs::ReadDirResult> {
        if cookie.0 != 0 {
            self.verify_cookie(verifier)?;
        }

        let rel = self.rel_path_from_handle(handle).await?;
        let abs = self.full_path(&rel);
        let dir_attr = self.get_attr(handle).await.ok();

        let mut entries = fs::read_dir(&abs).await.map_err(map_io_error)?;
        let mut names = Vec::new();
        while let Some(entry) = entries.next_entry().await.map_err(map_io_error)? {
            let name = entry.file_name();
            if name == OsStr::new(".") || name == OsStr::new("..") {
                continue;
            }
            names.push(name);
        }
        names.sort_by_key(|name| name.to_string_lossy().into_owned());

        let mut records = Vec::new();
        let budget = if max_bytes == 0 {
            usize::MAX
        } else {
            cmp::max(1, (max_bytes / ENTRY_ESTIMATE_BYTES) as usize)
        };
        let mut remaining = budget;
        for name in names.into_iter() {
            if remaining == 0 {
                break;
            }
            let child_rel = join_child(&rel, &name)?;
            let id = self.ensure_entry(child_rel).await;
            if cookie.0 != 0 && id <= cookie.0 {
                continue;
            }
            records.push(vfs::DirectoryEntry {
                cookie: vfs::DirectoryCookie(id),
                name: file_name_string(&name),
                fileid: id,
            });
            remaining -= 1;
        }

        Ok(vfs::ReadDirResult {
            directory_attr: dir_attr,
            cookie_verifier: self.cookie_verifier(),
            entries: records,
        })
    }

    async fn read_dir_plus(
        &self,
        handle: &vfs::FileHandle,
        cookie: vfs::DirectoryCookie,
        verifier: vfs::CookieVerifier,
        max_bytes: u32,
        max_handles: u32,
    ) -> vfs::VfsResult<vfs::ReadDirPlusResult> {
        if cookie.0 != 0 {
            self.verify_cookie(verifier)?;
        }

        let rel = self.rel_path_from_handle(handle).await?;
        let abs = self.full_path(&rel);
        let dir_attr = self.get_attr(handle).await.ok();

        let mut entries = fs::read_dir(&abs).await.map_err(map_io_error)?;
        let mut names = Vec::new();
        while let Some(entry) = entries.next_entry().await.map_err(map_io_error)? {
            let name = entry.file_name();
            if name == OsStr::new(".") || name == OsStr::new("..") {
                continue;
            }
            names.push(name);
        }
        names.sort_by_key(|name| name.to_string_lossy().into_owned());

        let mut info = Vec::new();
        for name in names.into_iter() {
            let child_rel = join_child(&rel, &name)?;
            let id = self.ensure_entry(child_rel.clone()).await;
            info.push((name, child_rel, id));
        }

        let byte_limit = if max_bytes == 0 {
            usize::MAX
        } else {
            cmp::max(1, (max_bytes / ENTRY_ESTIMATE_BYTES) as usize)
        };
        let handle_limit = if max_handles == 0 { usize::MAX } else { max_handles as usize };
        let mut remaining = cmp::min(byte_limit, handle_limit);

        let mut records = Vec::new();
        for (name, child_rel, id) in info.into_iter() {
            if remaining == 0 {
                break;
            }
            if cookie.0 != 0 && id <= cookie.0 {
                continue;
            }
            let child_abs = self.full_path(&child_rel);
            let meta = fs::symlink_metadata(&child_abs).await.map_err(map_io_error)?;
            records.push(vfs::DirectoryPlusEntry {
                cookie: vfs::DirectoryCookie(id),
                name: file_name_string(&name),
                fileid: id,
                handle: Some(Self::encode_handle(id)),
                attr: Some(Self::attr_from_meta(id, &meta)),
            });
            remaining -= 1;
        }

        Ok(vfs::ReadDirPlusResult {
            directory_attr: dir_attr,
            cookie_verifier: self.cookie_verifier(),
            entries: records,
        })
    }

    async fn fs_stat(&self, handle: &vfs::FileHandle) -> vfs::VfsResult<vfs::FsStat> {
        let attr = self.get_attr(handle).await.ok();
        // TODO: Make correct
        Ok(vfs::FsStat {
            total_bytes: 0,
            free_bytes: 0,
            available_bytes: 0,
            total_files: 0,
            free_files: 0,
            available_files: 0,
            invarsec: 0,
            file_attr: attr,
        })
    }

    async fn fs_info(&self, handle: &vfs::FileHandle) -> vfs::VfsResult<vfs::FsInfo> {
        let attr = self.get_attr(handle).await.ok();
        // TODO: Adjust these parameters as needed and move to constants.
        Ok(vfs::FsInfo {
            read_max: 1 << 20,
            read_pref: 64 << 10,
            read_multiple: 1,
            write_max: 1 << 20,
            write_pref: 64 << 10,
            write_multiple: 1,
            directory_pref: 4 << 10,
            max_file_size: u64::MAX,
            time_delta: vfs::FileTime { seconds: 1, nanos: 0 },
            properties: vfs::FsProperties::default(),
            file_attr: attr,
        })
    }

    async fn path_conf(&self, handle: &vfs::FileHandle) -> vfs::VfsResult<vfs::PathConfig> {
        let attr = self.get_attr(handle).await.ok();
        Ok(vfs::PathConfig {
            file_attr: attr,
            // TODO: Adjust these parameters as needed and move to constants.
            max_link: 1024,
            max_name: vfs::MAX_NAME_LEN as u32,
            no_trunc: true,
            chown_restricted: true,
            case_insensitive: false,
            case_preserving: true,
        })
    }

    // For what method?
    async fn commit(
        &self,
        handle: &vfs::FileHandle,
        offset: u64,
        count: u32,
    ) -> vfs::VfsResult<vfs::CommitResult> {
        let id = Self::decode_handle(handle)?;
        let rel = self.rel_path_from_id(id).await?;
        let abs = self.full_path(&rel);
        let mut meta = fs::metadata(&abs).await.map_err(map_io_error)?;

        let commit_whole_file = count == 0;
        if !commit_whole_file && offset > meta.len() {
            return Err(vfs::NfsError::Inval);
        }

        let requested_end = offset.saturating_add(count as u64);
        if commit_whole_file || requested_end > offset {
            let file = File::open(&abs).await.map_err(map_io_error)?;
            file.sync_data().await.map_err(map_io_error)?;
            meta = fs::metadata(&abs).await.map_err(map_io_error)?;
        }

        Ok(vfs::CommitResult {
            file_attr: Some(Self::attr_from_meta(id, &meta)),
            verifier: self.stable_verifier(),
        })
    }
}
