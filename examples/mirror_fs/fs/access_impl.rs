use async_trait::async_trait;

use nfs_mamont::vfs::access;
use nfs_mamont::vfs::file;

use super::MirrorFS;

#[async_trait]
impl access::Access for MirrorFS {
    async fn access(&self, args: access::Args) -> Result<access::Success, access::Fail> {
        let path = match self.path_for_handle(&args.file).await {
            Ok(path) => path,
            Err(error) => return Err(access::Fail { error, object_attr: None }),
        };
        let meta = match Self::metadata(&path) {
            Ok(meta) => meta,
            Err(error) => return Err(access::Fail { error, object_attr: None }),
        };
        let attr = Self::attr_from_metadata(&meta);
        let granted = Self::compute_access_mask(&attr, args.mask);
        Ok(access::Success { object_attr: Some(attr), access: granted })
    }
}

impl MirrorFS {
    /// Computes the access mask based on the file's mode bits (owner class).
    fn compute_access_mask(attr: &file::Attr, requested: access::Mask) -> access::Mask {
        let mode = attr.mode;
        let is_dir = matches!(attr.file_type, file::Type::Directory);
        let owner_r = mode & 0o400 != 0;
        let owner_w = mode & 0o200 != 0;
        let owner_x = mode & 0o100 != 0;

        let mut result = 0u32;
        if requested.contains(access::Mask::READ) && owner_r {
            result |= access::Mask::READ;
        }
        if requested.contains(access::Mask::LOOKUP) && is_dir && owner_x {
            result |= access::Mask::LOOKUP;
        }
        if requested.contains(access::Mask::MODIFY) && owner_w {
            result |= access::Mask::MODIFY;
        }
        if requested.contains(access::Mask::EXTEND) && owner_w {
            result |= access::Mask::EXTEND;
        }
        if requested.contains(access::Mask::DELETE) && owner_w {
            result |= access::Mask::DELETE;
        }
        if requested.contains(access::Mask::EXECUTE) && owner_x {
            result |= access::Mask::EXECUTE;
        }
        access::Mask::from_wire(result)
    }
}
