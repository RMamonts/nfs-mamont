use nfs_mamont::vfs::{self, write};

use super::MirrorFS;

impl write::Write for MirrorFS {
    async fn write(&self, args: write::Args) -> Result<write::Success, write::Fail> {
        let (_, path, attr) = match self.path_and_attr_for_handle(&args.file).await {
            Ok(path_and_attr) => path_and_attr,
            Err(error) => {
                return Err(write::Fail {
                    error,
                    wcc_data: vfs::WccData { before: None, after: None },
                });
            }
        };
        if let Err(error) = Self::validate_regular(&attr) {
            return Err(write::Fail {
                error,
                wcc_data: vfs::WccData {
                    before: Some(Self::wcc_attr_from_attr(&attr)),
                    after: Some(attr),
                },
            });
        }

        let before = Some(Self::wcc_attr_from_attr(&attr));
        let file = match self.get_cached_write_file(&path).await {
            Ok(file) => file,
            Err(error) => {
                return Err(write::Fail { error, wcc_data: vfs::WccData { before, after: None } });
            }
        };

        let stable = args.stable;
        let size = args.size as usize;
        let offset = args.offset;
        let data = args.data;
        let (after_attr, written) =
            match self.disk_io.write_from(file.clone(), offset, size, stable, data).await {
                Ok(result) => result,
                Err(error) => {
                    return Err(write::Fail {
                        error,
                        wcc_data: vfs::WccData { before, after: Self::file_attr(&path) },
                    });
                }
            };
        let _ = path;

        Ok(write::Success {
            file_wcc: vfs::WccData { before, after: Some(after_attr) },
            count: written,
            commited: stable,
            verifier: self.write_verifier(),
        })
    }
}
