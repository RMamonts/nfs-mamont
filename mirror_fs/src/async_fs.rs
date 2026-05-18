use std::fs::Metadata;
use std::io::{self, SeekFrom};
use std::path::{Path, PathBuf};

use tokio::fs::OpenOptions as TokioOpenOptions;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

pub struct File(tokio::fs::File);

impl File {
    pub async fn open(path: &Path) -> io::Result<Self> {
        TokioOpenOptions::new().read(true).open(path).await.map(Self)
    }

    pub async fn create(path: &Path) -> io::Result<Self> {
        TokioOpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)
            .await
            .map(Self)
    }

    pub async fn open_write(path: &Path) -> io::Result<Self> {
        TokioOpenOptions::new().write(true).open(path).await.map(Self)
    }

    pub async fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        AsyncReadExt::read(&mut self.0, buf).await
    }

    pub async fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        AsyncWriteExt::write_all(&mut self.0, buf).await
    }

    pub async fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        AsyncSeekExt::seek(&mut self.0, pos).await
    }

    pub async fn sync_all(&mut self) -> io::Result<()> {
        self.0.sync_all().await
    }

    pub async fn sync_data(&mut self) -> io::Result<()> {
        self.0.sync_data().await
    }
}

pub async fn metadata(path: &Path) -> io::Result<Metadata> {
    tokio::fs::metadata(path).await
}

pub async fn symlink_metadata(path: &Path) -> io::Result<Metadata> {
    tokio::fs::symlink_metadata(path).await
}

pub async fn create_dir(path: &Path) -> io::Result<()> {
    tokio::fs::create_dir(path).await
}

pub async fn remove_file(path: &Path) -> io::Result<()> {
    tokio::fs::remove_file(path).await
}

pub async fn rename(from: &Path, to: &Path) -> io::Result<()> {
    tokio::fs::rename(from, to).await
}

pub async fn hard_link(src: &Path, dst: &Path) -> io::Result<()> {
    tokio::fs::hard_link(src, dst).await
}

pub async fn set_permissions(path: &Path, perm: std::fs::Permissions) -> io::Result<()> {
    tokio::fs::set_permissions(path, perm).await
}

pub async fn read_dir(path: &Path) -> io::Result<ReadDir> {
    let dir = tokio::fs::read_dir(path).await?;
    Ok(ReadDir { inner: dir })
}

pub struct ReadDir {
    inner: tokio::fs::ReadDir,
}

impl ReadDir {
    pub async fn next_entry(&mut self) -> io::Result<Option<DirEntry>> {
        match self.inner.next_entry().await {
            Ok(Some(entry)) => Ok(Some(DirEntry(entry))),
            Ok(None) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

pub struct DirEntry(tokio::fs::DirEntry);

impl DirEntry {
    pub fn file_name(&self) -> std::path::PathBuf {
        self.0.file_name().into()
    }

    pub fn path(&self) -> std::path::PathBuf {
        self.0.path()
    }

    pub async fn metadata(&self) -> io::Result<Metadata> {
        self.0.metadata().await
    }
}

pub async fn set_times(path: &Path, atime: Option<std::time::SystemTime>, mtime: Option<std::time::SystemTime>) -> io::Result<()> {
    use std::fs::FileTimes;
    let mut times = FileTimes::new();
    if let Some(at) = atime {
        times = times.set_accessed(at);
    }
    if let Some(mt) = mtime {
        times = times.set_modified(mt);
    }
    std::fs::File::open(path)?.set_times(times)?;
    Ok(())
}

pub async fn remove_dir(path: &Path) -> io::Result<()> {
    tokio::fs::remove_dir(path).await
}

pub async fn remove_dir_all(path: &Path) -> io::Result<()> {
    tokio::fs::remove_dir_all(path).await
}

pub async fn canonicalize(path: &Path) -> io::Result<PathBuf> {
    tokio::fs::canonicalize(path).await
}

pub async fn symlink(target: &Path, link: &Path) -> io::Result<()> {
    tokio::fs::symlink(target, link).await
}

pub async fn copy(src: &Path, dst: &Path) -> io::Result<u64> {
    tokio::fs::copy(src, dst).await
}