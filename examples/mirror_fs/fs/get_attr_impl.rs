use nfs_mamont::vfs::get_attr;

use super::MirrorFS;

impl get_attr::GetAttr for MirrorFS {
    async fn get_attr(&self, args: get_attr::Args) -> Result<get_attr::Success, get_attr::Fail> {
        let path = match self.path_for_handle(&args.file).await {
            Ok(path) => path,
            Err(error) => {
                return Err(get_attr::Fail { error });
            }
        };
        match self.attr_for_path(&path).await {
            Ok(attr) => Ok(get_attr::Success { object: attr }),
            Err(error) => Err(get_attr::Fail { error }),
        }
    }
}
