use std::ffi::CString;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::io::RawFd;
use std::path::Path;
use std::sync::Arc;

use tokio::sync::{mpsc, oneshot};

use crate::uring::buffer::{DEFAULT_FIXED_BUFFER_COUNT, DEFAULT_FIXED_BUFFER_LEN};
use crate::uring::types::StatxData;
use crate::uring::worker::run_uring;
use crate::uring::{FixedBufferPool, UringRequest};

#[derive(Debug)]
pub struct UringExecutor {
    sender: mpsc::UnboundedSender<UringRequest>,
    max_fixed_len: usize,
}

impl UringExecutor {
    pub fn new(entries: u32) -> Option<Arc<Self>> {
        let mut ring = io_uring::IoUring::new(entries).ok()?;
        let mut probe = io_uring::Probe::new();
        ring.submitter().register_probe(&mut probe).ok()?;
        if !probe.is_supported(io_uring::opcode::Read::CODE)
            || !probe.is_supported(io_uring::opcode::Write::CODE)
            || !probe.is_supported(io_uring::opcode::OpenAt::CODE)
            || !probe.is_supported(io_uring::opcode::Statx::CODE)
            || !probe.is_supported(io_uring::opcode::Fsync::CODE)
        {
            return None;
        }
        let max_fixed_len = DEFAULT_FIXED_BUFFER_LEN;
        let pool = if probe.is_supported(io_uring::opcode::ReadFixed::CODE)
            && probe.is_supported(io_uring::opcode::WriteFixed::CODE)
        {
            FixedBufferPool::new(&mut ring, DEFAULT_FIXED_BUFFER_COUNT, max_fixed_len).ok()
        } else {
            None
        };
        let (sender, receiver) = mpsc::unbounded_channel();
        let executor =
            Arc::new(Self { sender, max_fixed_len: pool.as_ref().map_or(0, |_| max_fixed_len) });

        let _ = std::thread::Builder::new()
            .name("mirrorfs-uring".to_string())
            .spawn(move || run_uring(ring, receiver, pool));

        Some(executor)
    }

    pub fn max_io_len(&self) -> usize {
        if self.max_fixed_len == 0 {
            usize::MAX
        } else {
            self.max_fixed_len
        }
    }

    pub async fn fsync(&self, fd: RawFd, datasync: bool) -> io::Result<()> {
        let (reply, wait) = oneshot::channel();
        let flags = if datasync {
            io_uring::types::FsyncFlags::DATASYNC
        } else {
            io_uring::types::FsyncFlags::empty()
        };
        let request = UringRequest::Fsync { fd, flags, reply };

        if self.sender.send(request).is_err() {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "io_uring worker stopped"));
        }

        match wait.await {
            Ok(result) => result,
            Err(_) => Err(io::Error::new(io::ErrorKind::BrokenPipe, "io_uring worker stopped")),
        }
    }

    pub async fn write_at(&self, fd: RawFd, offset: u64, buffer: Vec<u8>) -> io::Result<usize> {
        let (reply, wait) = oneshot::channel();
        let request = UringRequest::Write { fd, offset, buffer, reply };

        if self.sender.send(request).is_err() {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "io_uring worker stopped"));
        }

        match wait.await {
            Ok(result) => result,
            Err(_) => Err(io::Error::new(io::ErrorKind::BrokenPipe, "io_uring worker stopped")),
        }
    }

    pub async fn read_at(&self, fd: RawFd, offset: u64, len: usize) -> io::Result<Vec<u8>> {
        let (reply, wait) = oneshot::channel();
        let request = UringRequest::Read { fd, offset, len, reply };

        if self.sender.send(request).is_err() {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "io_uring worker stopped"));
        }

        match wait.await {
            Ok(result) => result,
            Err(_) => Err(io::Error::new(io::ErrorKind::BrokenPipe, "io_uring worker stopped")),
        }
    }

    pub async fn open_at(&self, path: &Path, flags: i32, mode: u32) -> io::Result<RawFd> {
        let (reply, wait) = oneshot::channel();
        let c_path = CString::new(path.as_os_str().as_bytes())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "path contains null byte"))?;
        let request = UringRequest::Open { path: c_path, flags, mode, reply };

        if self.sender.send(request).is_err() {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "io_uring worker stopped"));
        }

        match wait.await {
            Ok(result) => result,
            Err(_) => Err(io::Error::new(io::ErrorKind::BrokenPipe, "io_uring worker stopped")),
        }
    }

    pub async fn statx(&self, path: &Path, follow: bool) -> io::Result<StatxData> {
        let (reply, wait) = oneshot::channel();
        let c_path = CString::new(path.as_os_str().as_bytes())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "path contains null byte"))?;
        let request = UringRequest::Statx { path: c_path, follow, reply };

        if self.sender.send(request).is_err() {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "io_uring worker stopped"));
        }

        match wait.await {
            Ok(result) => result,
            Err(_) => Err(io::Error::new(io::ErrorKind::BrokenPipe, "io_uring worker stopped")),
        }
    }
}
