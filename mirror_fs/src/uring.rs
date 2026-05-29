use std::collections::HashMap;
use std::ffi::CString;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::io::RawFd;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use io_uring::{opcode, squeue, types, IoUring, Probe};
use libc;
use nfs_mamont::AllocatorState;
use tokio::sync::{mpsc, oneshot};

#[derive(Debug)]
pub struct UringExecutor {
    sender: mpsc::UnboundedSender<UringRequest>,
    max_fixed_len: usize,
}

#[derive(Debug)]
pub struct UringPool {
    rings: Vec<Arc<UringExecutor>>,
    next: AtomicUsize,
}

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

impl UringExecutor {
    pub fn new(entries: u32) -> Option<Arc<Self>> {
        let mut ring = IoUring::new(entries).ok()?;
        let mut probe = Probe::new();
        ring.submitter().register_probe(&mut probe).ok()?;
        if !probe.is_supported(opcode::Read::CODE)
            || !probe.is_supported(opcode::Write::CODE)
            || !probe.is_supported(opcode::OpenAt::CODE)
            || !probe.is_supported(opcode::Statx::CODE)
            || !probe.is_supported(opcode::Fsync::CODE)
        {
            return None;
        }
        let max_fixed_len = DEFAULT_FIXED_BUFFER_LEN;
        let pool = if probe.is_supported(opcode::ReadFixed::CODE)
            && probe.is_supported(opcode::WriteFixed::CODE)
        {
            FixedBufferPool::new(&mut ring, DEFAULT_FIXED_BUFFER_COUNT, max_fixed_len).ok()
        } else {
            None
        };
        let (sender, receiver) = mpsc::unbounded_channel();
        let executor =
            Arc::new(Self { sender, max_fixed_len: pool.as_ref().map_or(0, |_| max_fixed_len) });

        thread::Builder::new()
            .name("mirrorfs-uring".to_string())
            .spawn(move || run_uring(ring, receiver, pool))
            .ok()?;

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
        let flags = if datasync { types::FsyncFlags::DATASYNC } else { types::FsyncFlags::empty() };
        let request = UringRequest::Fsync { fd, flags, reply };

        if self.sender.send(request).is_err() {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "io_uring worker stopped"));
        }

        match wait.await {
            Ok(result) => result,
            Err(_) => Err(io::Error::new(io::ErrorKind::BrokenPipe, "io_uring worker stopped")),
        }
    }

    pub async fn write_at(
        &self,
        fd: RawFd,
        offset: u64,
        buffer: Vec<u8>,
        alloc_state: Option<Arc<AllocatorState>>,
    ) -> io::Result<usize> {
        let (reply, wait) = oneshot::channel();
        let request = UringRequest::Write { fd, offset, buffer, reply, alloc_state };

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

impl UringPool {
    pub fn new(count: usize, entries: u32) -> Option<Arc<Self>> {
        if count == 0 {
            return None;
        }

        let mut rings = Vec::with_capacity(count);
        for _ in 0..count {
            rings.push(UringExecutor::new(entries)?);
        }

        Some(Arc::new(Self { rings, next: AtomicUsize::new(0) }))
    }

    pub fn pick(&self) -> Arc<UringExecutor> {
        let index = self.next.fetch_add(1, Ordering::Relaxed) % self.rings.len();
        self.rings[index].clone()
    }

    pub fn max_io_len(&self) -> usize {
        self.rings.first().map(|ring| ring.max_io_len()).unwrap_or(usize::MAX)
    }
}

enum UringRequest {
    Fsync { fd: RawFd, flags: types::FsyncFlags, reply: oneshot::Sender<io::Result<()>> },
    Write {
        fd: RawFd,
        offset: u64,
        buffer: Vec<u8>,
        reply: oneshot::Sender<io::Result<usize>>,
        alloc_state: Option<Arc<AllocatorState>>,
    },
    Read { fd: RawFd, offset: u64, len: usize, reply: oneshot::Sender<io::Result<Vec<u8>>> },
    Open { path: CString, flags: i32, mode: u32, reply: oneshot::Sender<io::Result<RawFd>> },
    Statx { path: CString, follow: bool, reply: oneshot::Sender<io::Result<StatxData>> },
}

enum InFlight {
    Fsync(oneshot::Sender<io::Result<()>>),
    Write {
        reply: oneshot::Sender<io::Result<usize>>,
        buffer: Vec<u8>,
        alloc_state: Option<Arc<AllocatorState>>,
    },
    Read { reply: oneshot::Sender<io::Result<Vec<u8>>>, buffer: Vec<u8> },
    ReadFixed { reply: oneshot::Sender<io::Result<Vec<u8>>>, index: usize, len: usize },
    Open { reply: oneshot::Sender<io::Result<RawFd>>, _path: CString },
    Statx { reply: oneshot::Sender<io::Result<StatxData>>, path: CString, statx: Box<libc::statx> },
}

const MAX_BATCH: usize = 64;
const BATCH_WAIT: Duration = Duration::from_micros(50);
const DEFAULT_FIXED_BUFFER_COUNT: usize = 128;
const DEFAULT_FIXED_BUFFER_LEN: usize = 64 * 1024;

struct FixedBufferPool {
    buffers: Vec<Vec<u8>>,
    free: Vec<usize>,
    len: usize,
}

impl FixedBufferPool {
    fn new(ring: &mut IoUring, count: usize, len: usize) -> io::Result<Self> {
        if count == 0 || len == 0 {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid fixed buffer pool"));
        }

        let mut buffers = Vec::with_capacity(count);
        let mut iovecs = Vec::with_capacity(count);
        for _ in 0..count {
            let mut buffer = vec![0u8; len];
            let iovec = libc::iovec { iov_base: buffer.as_mut_ptr().cast(), iov_len: buffer.len() };
            buffers.push(buffer);
            iovecs.push(iovec);
        }

        unsafe {
            ring.submitter().register_buffers(&iovecs)?;
        }

        let free = (0..count).rev().collect();
        Ok(Self { buffers, free, len })
    }

    fn take(&mut self) -> Option<usize> {
        self.free.pop()
    }

    fn release(&mut self, index: usize) {
        self.free.push(index);
    }
}

fn run_uring(
    mut ring: IoUring,
    mut receiver: mpsc::UnboundedReceiver<UringRequest>,
    mut pool: Option<FixedBufferPool>,
) {
    let mut next_id: u64 = 1;

    loop {
        let Some(request) = receiver.blocking_recv() else {
            break;
        };

        let mut batch = vec![request];
        let start = Instant::now();
        while batch.len() < MAX_BATCH && start.elapsed() < BATCH_WAIT {
            match receiver.try_recv() {
                Ok(request) => batch.push(request),
                Err(_) => thread::yield_now(),
            }
        }

        let mut inflight: HashMap<u64, InFlight> = HashMap::new();
        let mut submitted = 0usize;

        {
            let mut submission = ring.submission();
            for request in batch {
                match request {
                    UringRequest::Fsync { fd, flags, reply } => {
                        let user_data = next_id;
                        next_id = next_id.wrapping_add(1);
                        let entry = opcode::Fsync::new(types::Fd(fd))
                            .flags(flags)
                            .build()
                            .flags(squeue::Flags::ASYNC)
                            .user_data(user_data);

                        if unsafe { submission.push(&entry) }.is_err() {
                            let _ =
                                reply.send(Err(io::Error::other("io_uring submission queue full")));
                            continue;
                        }

                        inflight.insert(user_data, InFlight::Fsync(reply));
                        submitted += 1;
                    }
                    UringRequest::Write { fd, offset, buffer, reply, alloc_state } => {
                        let user_data = next_id;
                        next_id = next_id.wrapping_add(1);
                        let entry =
                            opcode::Write::new(types::Fd(fd), buffer.as_ptr(), buffer.len() as u32)
                                .offset(offset)
                                .build()
                                .flags(squeue::Flags::ASYNC)
                                .user_data(user_data);

                        if unsafe { submission.push(&entry) }.is_err() {
                            let _ =
                                reply.send(Err(io::Error::other("io_uring submission queue full")));
                            if let Some(state) = alloc_state {
                                let _ = state.pool.push(buffer.into_boxed_slice());
                                state.semaphore.add_permits(1);
                            }
                            continue;
                        }

                        inflight.insert(user_data, InFlight::Write { reply, buffer, alloc_state });
                        submitted += 1;
                    }
                    UringRequest::Read { fd, offset, len, reply } => {
                        if let Some(pool) = pool.as_mut() {
                            if len <= pool.len {
                                if let Some(index) = pool.take() {
                                    let user_data = next_id;
                                    next_id = next_id.wrapping_add(1);
                                    let entry = opcode::ReadFixed::new(
                                        types::Fd(fd),
                                        pool.buffers[index].as_mut_ptr(),
                                        len as u32,
                                        index as u16,
                                    )
                                    .offset(offset)
                                    .build()
                                    .flags(squeue::Flags::ASYNC)
                                    .user_data(user_data);

                                    if unsafe { submission.push(&entry) }.is_err() {
                                        pool.release(index);
                                        let _ = reply.send(Err(io::Error::other(
                                            "io_uring submission queue full",
                                        )));
                                        continue;
                                    }

                                    inflight.insert(
                                        user_data,
                                        InFlight::ReadFixed { reply, index, len },
                                    );
                                    submitted += 1;
                                    continue;
                                }
                            }
                        }

                        let user_data = next_id;
                        next_id = next_id.wrapping_add(1);
                        let mut buffer = vec![0u8; len];
                        let entry = opcode::Read::new(
                            types::Fd(fd),
                            buffer.as_mut_ptr(),
                            buffer.len() as u32,
                        )
                        .offset(offset)
                        .build()
                        .flags(squeue::Flags::ASYNC)
                        .user_data(user_data);

                        if unsafe { submission.push(&entry) }.is_err() {
                            let _ =
                                reply.send(Err(io::Error::other("io_uring submission queue full")));
                            continue;
                        }

                        inflight.insert(user_data, InFlight::Read { reply, buffer });
                        submitted += 1;
                    }
                    UringRequest::Open { path, flags, mode, reply } => {
                        let user_data = next_id;
                        next_id = next_id.wrapping_add(1);
                        let entry = opcode::OpenAt::new(types::Fd(libc::AT_FDCWD), path.as_ptr())
                            .flags(flags)
                            .mode(mode)
                            .build()
                            .flags(squeue::Flags::ASYNC)
                            .user_data(user_data);

                        if unsafe { submission.push(&entry) }.is_err() {
                            let _ =
                                reply.send(Err(io::Error::other("io_uring submission queue full")));
                            continue;
                        }

                        inflight.insert(user_data, InFlight::Open { reply, _path: path });
                        submitted += 1;
                    }
                    UringRequest::Statx { path, follow, reply } => {
                        let user_data = next_id;
                        next_id = next_id.wrapping_add(1);
                        let mut statx = Box::new(unsafe { std::mem::zeroed::<libc::statx>() });
                        let flags = if follow { 0 } else { libc::AT_SYMLINK_NOFOLLOW };
                        let entry = opcode::Statx::new(
                            types::Fd(libc::AT_FDCWD),
                            path.as_ptr(),
                            statx.as_mut() as *mut libc::statx as *mut types::statx,
                        )
                        .flags(flags)
                        .mask(libc::STATX_BASIC_STATS)
                        .build()
                        .flags(squeue::Flags::ASYNC)
                        .user_data(user_data);

                        if unsafe { submission.push(&entry) }.is_err() {
                            let _ =
                                reply.send(Err(io::Error::other("io_uring submission queue full")));
                            continue;
                        }

                        inflight.insert(user_data, InFlight::Statx { reply, path, statx });
                        submitted += 1;
                    }
                }
            }
        }

        if submitted == 0 {
            continue;
        }

        if let Err(error) = ring.submit_and_wait(1) {
            fail_inflight(inflight, error, &mut pool);
            continue;
        }

        while !inflight.is_empty() {
            {
                let completions = ring.completion();
                for cqe in completions {
                    let Some(request) = inflight.remove(&cqe.user_data()) else {
                        continue;
                    };

                    let result = if cqe.result() < 0 {
                        Err(io::Error::from_raw_os_error(-cqe.result()))
                    } else {
                        Ok(cqe.result() as usize)
                    };

                    match request {
                        InFlight::Fsync(reply) => {
                            let _ = reply.send(result.map(|_| ()));
                        }
                        InFlight::Write { reply, buffer, alloc_state } => {
                            let max = buffer.len();
                            let result = result.and_then(|bytes| {
                                if bytes > max {
                                    Err(io::Error::new(
                                        io::ErrorKind::InvalidData,
                                        "write overflow",
                                    ))
                                } else {
                                    Ok(bytes)
                                }
                            });
                            if let Some(state) = alloc_state {
                                if state.pool.push(buffer.into_boxed_slice()).is_ok() {
                                    state.semaphore.add_permits(1);
                                }
                            }
                            let _ = reply.send(result);
                        }

                        InFlight::Read { reply, mut buffer } => {
                            let max = buffer.len();
                            let result = result.and_then(|bytes| {
                                if bytes > max {
                                    Err(io::Error::new(io::ErrorKind::InvalidData, "read overflow"))
                                } else {
                                    buffer.truncate(bytes);
                                    Ok(buffer)
                                }
                            });
                            let _ = reply.send(result);
                        }
                        InFlight::ReadFixed { reply, index, len } => {
                            let result = result.and_then(|bytes| {
                                if bytes > len {
                                    Err(io::Error::new(io::ErrorKind::InvalidData, "read overflow"))
                                } else {
                                    Ok(bytes)
                                }
                            });
                            let output = match (pool.as_mut(), result) {
                                (Some(pool), Ok(bytes)) => {
                                    let mut data = vec![0u8; bytes];
                                    data.copy_from_slice(&pool.buffers[index][..bytes]);
                                    pool.release(index);
                                    Ok(data)
                                }
                                (Some(pool), Err(error)) => {
                                    pool.release(index);
                                    Err(error)
                                }
                                (None, Ok(bytes)) => Ok(vec![0u8; bytes]),
                                (None, Err(error)) => Err(error),
                            };
                            let _ = reply.send(output);
                        }
                        InFlight::Open { reply, .. } => {
                            let result = result.map(|value| value as RawFd);
                            let _ = reply.send(result);
                        }
                        InFlight::Statx { reply, path, statx } => {
                            let result = result.and_then(|_| statx_to_data(&path, &statx));
                            let _ = reply.send(result);
                        }
                    }
                }
            }

            if inflight.is_empty() {
                break;
            }

            if let Err(error) = ring.submit_and_wait(1) {
                fail_inflight(inflight, error, &mut pool);
                break;
            }
        }
    }
}

fn fail_inflight(
    inflight: HashMap<u64, InFlight>,
    error: io::Error,
    pool: &mut Option<FixedBufferPool>,
) {
    let error = Arc::new(error);
    for (_, request) in inflight {
        match request {
            InFlight::Fsync(reply) => {
                let _ = reply.send(Err(io::Error::new(error.kind(), error.to_string())));
            }
            InFlight::Write { reply, buffer, alloc_state } => {
                if let Some(state) = alloc_state {
                    if state.pool.push(buffer.into_boxed_slice()).is_ok() {
                        state.semaphore.add_permits(1);
                    }
                }
                let _ = reply.send(Err(io::Error::new(error.kind(), error.to_string())));
            }
            InFlight::Read { reply, .. } => {
                let _ = reply.send(Err(io::Error::new(error.kind(), error.to_string())));
            }
            InFlight::ReadFixed { reply, index, .. } => {
                if let Some(pool) = pool.as_mut() {
                    pool.release(index);
                }
                let _ = reply.send(Err(io::Error::new(error.kind(), error.to_string())));
            }
            InFlight::Open { reply, .. } => {
                let _ = reply.send(Err(io::Error::new(error.kind(), error.to_string())));
            }
            InFlight::Statx { reply, .. } => {
                let _ = reply.send(Err(io::Error::new(error.kind(), error.to_string())));
            }
        }
    }
}

fn statx_to_data(_path: &CString, statx: &libc::statx) -> io::Result<StatxData> {
    Ok(StatxData {
        mode: statx.stx_mode as u32,
        nlink: statx.stx_nlink,
        uid: statx.stx_uid,
        gid: statx.stx_gid,
        size: statx.stx_size,
        blocks: statx.stx_blocks,
        dev_major: statx.stx_dev_major,
        dev_minor: statx.stx_dev_minor,
        ino: statx.stx_ino,
        atime_sec: statx.stx_atime.tv_sec,
        atime_nsec: statx.stx_atime.tv_nsec as i64,
        mtime_sec: statx.stx_mtime.tv_sec,
        mtime_nsec: statx.stx_mtime.tv_nsec as i64,
        ctime_sec: statx.stx_ctime.tv_sec,
        ctime_nsec: statx.stx_ctime.tv_nsec as i64,
    })
}
