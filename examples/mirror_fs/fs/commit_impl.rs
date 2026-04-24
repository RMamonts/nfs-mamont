use nfs_mamont::vfs::commit;

use super::*;

impl commit::Commit for MirrorFS {
    async fn commit(&self, args: commit::Args) -> Result<commit::Success, commit::Fail> {
        let (_, path, attr) = match self.path_and_attr_for_handle(&args.file).await {
            Ok(path_and_attr) => path_and_attr,
            Err(error) => {
                return Err(commit::Fail {
                    error,
                    file_wcc: vfs::WccData { before: None, after: None },
                });
            }
        };
        if let Err(error) = Self::validate_regular(&attr) {
            return Err(commit::Fail {
                error,
                file_wcc: vfs::WccData {
                    before: Some(Self::wcc_attr_from_attr(&attr)),
                    after: Some(attr),
                },
            });
        }
        let before = Some(Self::wcc_attr_from_attr(&attr));

        let file = match self.get_cached_write_file(&path).await {
            Ok(file) => file,
            Err(error) => {
                return Err(commit::Fail { error, file_wcc: vfs::WccData { before, after: None } })
            }
        };

        if let Err(error) = self.disk_io.fsync(file.clone(), false).await {
            return Err(commit::Fail {
                error,
                file_wcc: vfs::WccData { before, after: Self::file_attr(&path) },
            });
        }

        self.clear_pending_unstable_write(&path).await;

        Ok(commit::Success {
            file_wcc: vfs::WccData { before, after: Some(attr) },
            verifier: self.write_verifier(),
        })
    }
}
