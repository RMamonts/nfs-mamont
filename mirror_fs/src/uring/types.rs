use std::ffi::CString;
use std::io;
use std::os::unix::io::RawFd;

use tokio::sync::oneshot;

#[derive(Clone, Debug)]
pub struct StatxData {
    pub mode: u32,
    pub nlink: u32,
    pub uid: u32,
    pub gid: u32,
    pub size: u64,
    pub blocks: u64,
    pub dev_major: u32,
    pub dev_minor: u32,
    pub ino: u64,
    pub atime_sec: i64,
    pub atime_nsec: i64,
    pub mtime_sec: i64,
    pub mtime_nsec: i64,
    pub ctime_sec: i64,
    pub ctime_nsec: i64,
}

#[derive(Debug)]
pub enum UringRequest {
    Fsync { fd: RawFd, flags: io_uring::types::FsyncFlags, reply: oneshot::Sender<io::Result<()>> },
    Write { fd: RawFd, offset: u64, buffer: Vec<u8>, reply: oneshot::Sender<io::Result<usize>> },
    Read { fd: RawFd, offset: u64, len: usize, reply: oneshot::Sender<io::Result<Vec<u8>>> },
    Open { path: CString, flags: i32, mode: u32, reply: oneshot::Sender<io::Result<RawFd>> },
    Statx { path: CString, follow: bool, reply: oneshot::Sender<io::Result<StatxData>> },
}

#[derive(Debug)]
pub enum InFlight {
    Fsync(oneshot::Sender<io::Result<()>>),
    Write { reply: oneshot::Sender<io::Result<usize>>, buffer: Vec<u8> },
    WriteFixed { reply: oneshot::Sender<io::Result<usize>>, index: usize, len: usize },
    Read { reply: oneshot::Sender<io::Result<Vec<u8>>>, buffer: Vec<u8> },
    ReadFixed { reply: oneshot::Sender<io::Result<Vec<u8>>>, index: usize, len: usize },
    Open { reply: oneshot::Sender<io::Result<RawFd>>, _path: CString },
    Statx { reply: oneshot::Sender<io::Result<StatxData>>, path: CString, statx: Box<libc::statx> },
}
