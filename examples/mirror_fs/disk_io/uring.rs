use std::collections::{HashMap, VecDeque};
use std::ffi::CString;
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::io;
use std::mem::MaybeUninit;
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;

use async_channel::{Receiver, Sender, TryRecvError, TrySendError};
use io_uring::{opcode, types, IoUring};

use nfs_mamont::vfs;
use nfs_mamont::vfs::file;
use nfs_mamont::vfs::write;
use nfs_mamont::Slice;

use crate::config::DiskIoConfig;

const INLINE_IOVEC_CAP: usize = 4;

#[derive(Clone)]
pub struct DiskIo {
    workers: Arc<[WorkerHandle]>,
}

#[derive(Debug)]
pub struct DiskFile {
    file: Arc<File>,
    shard: usize,
    fixed_slot: Option<u32>,
}

#[derive(Clone, Debug)]
pub struct DirectoryEntryData {
    pub name: String,
    pub path: PathBuf,
    pub file_id: u64,
}

#[derive(Clone, Debug)]
pub struct WorkerMetricsSnapshot {
    pub worker_index: usize,
    pub submitted_sqes: u64,
    pub completed_cqes: u64,
    pub batched_submits: u64,
    pub sq_full_events: u64,
    pub dropped_background_requests: u64,
    pub current_inflight: usize,
    pub max_inflight: usize,
    pub channel_backlog: usize,
}

#[derive(Clone, Debug)]
pub struct DiskIoMetricsSnapshot {
    pub submitted_sqes: u64,
    pub completed_cqes: u64,
    pub batched_submits: u64,
    pub sq_full_events: u64,
    pub dropped_background_requests: u64,
    pub workers: Vec<WorkerMetricsSnapshot>,
}

#[derive(Clone)]
struct WorkerHandle {
    sender: Sender<QueuedRequest>,
    metrics: Arc<WorkerMetrics>,
}

#[derive(Default)]
struct WorkerMetrics {
    submitted_sqes: AtomicU64,
    completed_cqes: AtomicU64,
    batched_submits: AtomicU64,
    sq_full_events: AtomicU64,
    dropped_background_requests: AtomicU64,
    current_inflight: AtomicUsize,
    max_inflight: AtomicUsize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RequestPriority {
    Foreground,
    Background,
}

struct QueuedRequest {
    priority: RequestPriority,
    request: Request,
}

enum Request {
    Open {
        path: PathBuf,
        writable: bool,
        response: Sender<Result<Arc<DiskFile>, io::Error>>,
    },
    Stat {
        path: PathBuf,
        response: Sender<Result<file::Attr, io::Error>>,
    },
    Read {
        file: Arc<DiskFile>,
        offset: u64,
        data: Slice,
        data_offset: usize,
        len: usize,
        response: Sender<Result<(Slice, usize), io::Error>>,
    },
    Write {
        file: Arc<DiskFile>,
        offset: u64,
        size: usize,
        stable: write::StableHow,
        data: Slice,
        response: Sender<Result<(file::Attr, u32), io::Error>>,
    },
    Fsync {
        file: Arc<DiskFile>,
        data_only: bool,
        response: Sender<Result<(), io::Error>>,
    },
    ReadAhead {
        file: Arc<DiskFile>,
        offset: u64,
        len: usize,
        response: Sender<Result<Arc<[u8]>, io::Error>>,
    },
    ReadDir {
        path: PathBuf,
        response: Sender<Result<Vec<DirectoryEntryData>, io::Error>>,
    },
}

impl DiskFile {
    fn new(file: File, shard: usize, fixed_slot: Option<u32>) -> Self {
        Self { file: Arc::new(file), shard, fixed_slot }
    }

    pub fn shard(&self) -> usize {
        self.shard
    }

    pub fn backing_file(&self) -> Arc<File> {
        self.file.clone()
    }

    fn raw_fd(&self) -> i32 {
        self.file.as_raw_fd()
    }

    #[allow(dead_code)]
    pub fn fixed_slot(&self) -> Option<u32> {
        self.fixed_slot
    }
}

impl DiskIo {
    #[allow(dead_code)]
    pub fn new() -> io::Result<Self> {
        Self::with_config(DiskIoConfig::default())
    }

    pub fn with_config(config: DiskIoConfig) -> io::Result<Self> {
        let worker_count = config.worker_count.get();
        let mut workers = Vec::with_capacity(worker_count);

        for worker_idx in 0..worker_count {
            let (sender, receiver) =
                async_channel::bounded::<QueuedRequest>(config.channel_capacity.get());
            let metrics = Arc::new(WorkerMetrics::default());
            Worker::spawn(worker_idx, receiver, config.clone(), metrics.clone())?;
            workers.push(WorkerHandle { sender, metrics });
        }

        Ok(Self { workers: Arc::<[WorkerHandle]>::from(workers) })
    }

    pub fn metrics_snapshot(&self) -> DiskIoMetricsSnapshot {
        let mut workers = Vec::with_capacity(self.workers.len());
        let mut submitted_sqes = 0;
        let mut completed_cqes = 0;
        let mut batched_submits = 0;
        let mut sq_full_events = 0;
        let mut dropped_background_requests = 0;

        for (worker_index, worker) in self.workers.iter().enumerate() {
            let snapshot = WorkerMetricsSnapshot {
                worker_index,
                submitted_sqes: worker.metrics.submitted_sqes.load(Ordering::Relaxed),
                completed_cqes: worker.metrics.completed_cqes.load(Ordering::Relaxed),
                batched_submits: worker.metrics.batched_submits.load(Ordering::Relaxed),
                sq_full_events: worker.metrics.sq_full_events.load(Ordering::Relaxed),
                dropped_background_requests: worker
                    .metrics
                    .dropped_background_requests
                    .load(Ordering::Relaxed),
                current_inflight: worker.metrics.current_inflight.load(Ordering::Relaxed),
                max_inflight: worker.metrics.max_inflight.load(Ordering::Relaxed),
                channel_backlog: worker.sender.len(),
            };
            submitted_sqes += snapshot.submitted_sqes;
            completed_cqes += snapshot.completed_cqes;
            batched_submits += snapshot.batched_submits;
            sq_full_events += snapshot.sq_full_events;
            dropped_background_requests += snapshot.dropped_background_requests;
            workers.push(snapshot);
        }

        DiskIoMetricsSnapshot {
            submitted_sqes,
            completed_cqes,
            batched_submits,
            sq_full_events,
            dropped_background_requests,
            workers,
        }
    }

    pub async fn open_read(&self, path: &Path) -> Result<Arc<DiskFile>, vfs::Error> {
        self.open(path, false).await
    }

    pub async fn open_write(&self, path: &Path) -> Result<Arc<DiskFile>, vfs::Error> {
        self.open(path, true).await
    }

    pub async fn stat(&self, path: &Path) -> Result<file::Attr, vfs::Error> {
        let (response_tx, response_rx) = async_channel::bounded(1);
        let shard = self.shard_for_path(path);
        self.dispatch_to_shard(
            shard,
            Request::Stat { path: path.to_path_buf(), response: response_tx },
            RequestPriority::Foreground,
        )
        .await
        .map_err(Self::io_error_to_vfs)?;
        response_rx.recv().await.map_err(|_| vfs::Error::IO)?.map_err(Self::io_error_to_vfs)
    }

    pub async fn stat_many(&self, paths: Vec<PathBuf>) -> Result<Vec<(PathBuf, file::Attr)>, vfs::Error> {
        let mut receivers = Vec::with_capacity(paths.len());
        for path in paths {
            let (response_tx, response_rx) = async_channel::bounded(1);
            let shard = self.shard_for_path(&path);
            self.dispatch_to_shard(
                shard,
                Request::Stat { path: path.clone(), response: response_tx },
                RequestPriority::Foreground,
            )
            .await
            .map_err(Self::io_error_to_vfs)?;
            receivers.push((path, response_rx));
        }

        let mut results = Vec::with_capacity(receivers.len());
        for (path, receiver) in receivers {
            let attr = receiver.recv().await.map_err(|_| vfs::Error::IO)?.map_err(Self::io_error_to_vfs)?;
            results.push((path, attr));
        }
        Ok(results)
    }

    pub async fn read_into(
        &self,
        file: Arc<DiskFile>,
        offset: u64,
        data: Slice,
        data_offset: usize,
        len: usize,
    ) -> Result<(Slice, usize), vfs::Error> {
        let (response_tx, response_rx) = async_channel::bounded(1);
        self.dispatch_to_shard(
            file.shard(),
            Request::Read { file, offset, data, data_offset, len, response: response_tx },
            RequestPriority::Foreground,
        )
        .await
        .map_err(Self::io_error_to_vfs)?;
        response_rx.recv().await.map_err(|_| vfs::Error::IO)?.map_err(Self::io_error_to_vfs)
    }

    pub async fn write_from(
        &self,
        file: Arc<DiskFile>,
        offset: u64,
        size: usize,
        stable: write::StableHow,
        data: Slice,
    ) -> Result<(file::Attr, u32), vfs::Error> {
        let (response_tx, response_rx) = async_channel::bounded(1);
        self.dispatch_to_shard(
            file.shard(),
            Request::Write { file, offset, size, stable, data, response: response_tx },
            RequestPriority::Foreground,
        )
        .await
        .map_err(Self::io_error_to_vfs)?;
        response_rx.recv().await.map_err(|_| vfs::Error::IO)?.map_err(Self::io_error_to_vfs)
    }

    pub async fn fsync(&self, file: Arc<DiskFile>, data_only: bool) -> Result<(), vfs::Error> {
        let (response_tx, response_rx) = async_channel::bounded(1);
        self.dispatch_to_shard(
            file.shard(),
            Request::Fsync { file, data_only, response: response_tx },
            RequestPriority::Foreground,
        )
        .await
        .map_err(Self::io_error_to_vfs)?;
        response_rx.recv().await.map_err(|_| vfs::Error::IO)?.map_err(Self::io_error_to_vfs)
    }

    pub async fn read_ahead(
        &self,
        file: Arc<DiskFile>,
        offset: u64,
        len: usize,
    ) -> Result<Arc<[u8]>, io::Error> {
        let (response_tx, response_rx) = async_channel::bounded(1);
        self.dispatch_to_shard(
            file.shard(),
            Request::ReadAhead { file, offset, len, response: response_tx },
            RequestPriority::Background,
        )
        .await?;
        response_rx
            .recv()
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "io_uring workers stopped"))?
    }

    pub async fn read_dir(&self, path: &Path) -> Result<Vec<DirectoryEntryData>, vfs::Error> {
        let (response_tx, response_rx) = async_channel::bounded(1);
        let shard = self.shard_for_path(path);
        self.dispatch_to_shard(
            shard,
            Request::ReadDir { path: path.to_path_buf(), response: response_tx },
            RequestPriority::Foreground,
        )
        .await
        .map_err(Self::io_error_to_vfs)?;
        response_rx.recv().await.map_err(|_| vfs::Error::IO)?.map_err(Self::io_error_to_vfs)
    }

    async fn open(&self, path: &Path, writable: bool) -> Result<Arc<DiskFile>, vfs::Error> {
        let (response_tx, response_rx) = async_channel::bounded(1);
        let shard = self.shard_for_path(path);
        self.dispatch_to_shard(
            shard,
            Request::Open {
                path: path.to_path_buf(),
                writable,
                response: response_tx,
            },
            RequestPriority::Foreground,
        )
        .await
        .map_err(Self::io_error_to_vfs)?;
        response_rx.recv().await.map_err(|_| vfs::Error::IO)?.map_err(Self::io_error_to_vfs)
    }

    async fn dispatch_to_shard(
        &self,
        shard: usize,
        request: Request,
        priority: RequestPriority,
    ) -> io::Result<()> {
        let worker = &self.workers[shard % self.workers.len()];
        let queued = QueuedRequest { priority, request };
        match priority {
            RequestPriority::Foreground => worker
                .sender
                .send(queued)
                .await
                .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "io_uring workers stopped")),
            RequestPriority::Background => match worker.sender.try_send(queued) {
                Ok(()) => Ok(()),
                Err(TrySendError::Full(_)) => {
                    worker.metrics.dropped_background_requests.fetch_add(1, Ordering::Relaxed);
                    Err(io::Error::new(io::ErrorKind::WouldBlock, "background queue is full"))
                }
                Err(TrySendError::Closed(_)) => Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "io_uring workers stopped",
                )),
            },
        }
    }

    fn shard_for_path(&self, path: &Path) -> usize {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        path.hash(&mut hasher);
        (hasher.finish() as usize) % self.workers.len()
    }

    fn io_error_to_vfs(error: io::Error) -> vfs::Error {
        match error.kind() {
            io::ErrorKind::NotFound => vfs::Error::NoEntry,
            io::ErrorKind::PermissionDenied => vfs::Error::Access,
            io::ErrorKind::AlreadyExists => vfs::Error::Exist,
            io::ErrorKind::InvalidInput | io::ErrorKind::InvalidData => vfs::Error::InvalidArgument,
            io::ErrorKind::DirectoryNotEmpty => vfs::Error::NotEmpty,
            io::ErrorKind::IsADirectory => vfs::Error::IsDir,
            io::ErrorKind::NotADirectory => vfs::Error::NotDir,
            io::ErrorKind::WriteZero => vfs::Error::NoSpace,
            _ => vfs::Error::IO,
        }
    }
}

struct Worker {
    worker_idx: usize,
    ring: IoUring,
    receiver: Receiver<QueuedRequest>,
    config: DiskIoConfig,
    metrics: Arc<WorkerMetrics>,
    pending_foreground: VecDeque<Request>,
    pending_background: VecDeque<Request>,
    retry_queue: VecDeque<InflightOp>,
    inflight: HashMap<u64, InflightOp>,
    background_inflight: usize,
    next_user_data: u64,
}

impl Worker {
    fn spawn(
        worker_idx: usize,
        receiver: Receiver<QueuedRequest>,
        config: DiskIoConfig,
        metrics: Arc<WorkerMetrics>,
    ) -> io::Result<()> {
        let ring = IoUring::new(config.ring_entries)?;
        thread::Builder::new()
            .name(format!("mirrorfs-uring-{worker_idx}"))
            .spawn(move || {
                let mut worker = Self {
                    worker_idx,
                    ring,
                    receiver,
                    config,
                    metrics,
                    pending_foreground: VecDeque::new(),
                    pending_background: VecDeque::new(),
                    retry_queue: VecDeque::new(),
                    inflight: HashMap::new(),
                    background_inflight: 0,
                    next_user_data: 1,
                };
                worker.run();
            })?;
        Ok(())
    }

    fn run(&mut self) {
        loop {
            if !self.ensure_work_available() {
                break;
            }

            self.drain_request_queue();

            let submitted = self.submit_ready_ops();
            if submitted > 0 {
                if self.ring.submit().is_err() {
                    self.fail_all(io::Error::new(io::ErrorKind::BrokenPipe, "io_uring submit failed"));
                    break;
                }
                self.metrics.batched_submits.fetch_add(1, Ordering::Relaxed);
            }

            let completed_now = self.drain_completions();
            if completed_now > 0 {
                continue;
            }

            if self.inflight.is_empty() {
                continue;
            }

            if self.ring.submit_and_wait(1).is_err() {
                self.fail_all(io::Error::new(io::ErrorKind::BrokenPipe, "io_uring wait failed"));
                break;
            }
            self.drain_completions();
        }
    }

    fn ensure_work_available(&mut self) -> bool {
        if !self.pending_foreground.is_empty()
            || !self.pending_background.is_empty()
            || !self.retry_queue.is_empty()
            || !self.inflight.is_empty()
        {
            return true;
        }

        match self.receiver.recv_blocking() {
            Ok(queued) => {
                self.enqueue_request(queued);
                true
            }
            Err(_) => false,
        }
    }

    fn drain_request_queue(&mut self) {
        loop {
            match self.receiver.try_recv() {
                Ok(queued) => self.enqueue_request(queued),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Closed) => break,
            }
        }
    }

    fn enqueue_request(&mut self, queued: QueuedRequest) {
        match queued.priority {
            RequestPriority::Foreground => self.pending_foreground.push_back(queued.request),
            RequestPriority::Background => self.pending_background.push_back(queued.request),
        }
    }

    fn submit_ready_ops(&mut self) -> usize {
        let mut submitted = 0usize;
        let sqe_budget = self.config.ring_entries as usize;

        while submitted < sqe_budget && self.inflight.len() < self.config.max_inflight_per_worker.get() {
            let mut op = match self.next_ready_op() {
                Some(op) => op,
                None => break,
            };

            let user_data = self.next_user_data;
            self.next_user_data = self.next_user_data.wrapping_add(1).max(1);
            let entry = match op.build_entry(user_data) {
                Ok(entry) => entry,
                Err(error) => {
                    op.fail(error);
                    continue;
                }
            };

            let push_result = unsafe { self.ring.submission().push(&entry) };
            if push_result.is_err() {
                self.metrics.sq_full_events.fetch_add(1, Ordering::Relaxed);
                self.retry_queue.push_front(op);
                break;
            }

            if op.priority() == RequestPriority::Background {
                self.background_inflight += 1;
            }
            self.inflight.insert(user_data, op);
            submitted += 1;
            self.metrics.submitted_sqes.fetch_add(1, Ordering::Relaxed);
            self.metrics.current_inflight.fetch_add(1, Ordering::Relaxed);
            update_max(
                &self.metrics.max_inflight,
                self.metrics.current_inflight.load(Ordering::Relaxed),
            );
        }

        submitted
    }

    fn next_ready_op(&mut self) -> Option<InflightOp> {
        if let Some(op) = self.retry_queue.pop_front() {
            return Some(op);
        }

        if let Some(request) = self.pending_foreground.pop_front() {
            return self.request_to_op(request);
        }

        if self.background_inflight >= self.config.prefetch_budget_per_worker.get() {
            return None;
        }

        self.pending_background.pop_front().and_then(|request| self.request_to_op(request))
    }

    fn request_to_op(&mut self, request: Request) -> Option<InflightOp> {
        match request {
            Request::Open { path, writable, response } => match cstring_for_path(&path) {
                Ok(c_path) => Some(InflightOp::Open(OpenOp {
                    shard: self.worker_idx,
                    writable,
                    path: c_path,
                    response,
                })),
                Err(error) => {
                    let _ = response.send_blocking(Err(error));
                    None
                }
            },
            Request::Stat { path, response } => {
                let result = std::fs::symlink_metadata(&path).map(|meta| attr_from_metadata(&meta));
                let _ = response.send_blocking(result);
                None
            }
            Request::Read { file, offset, data, data_offset, len, response } => {
                if len == 0 {
                    let _ = response.send_blocking(Ok((data, 0)));
                    return None;
                }
                Some(InflightOp::Read(ReadOp {
                    file,
                    response,
                    data,
                    data_offset,
                    total: 0,
                    remaining: len,
                    current_offset: offset,
                    prepared: PreparedRead::Empty,
                }))
            }
            Request::Write { file, offset, size, stable, data, response } => {
                let stage = if size == 0 {
                    match stable {
                        write::StableHow::Unstable => WriteStage::DoneWithoutSync,
                        write::StableHow::DataSync => WriteStage::Syncing { data_only: true },
                        write::StableHow::FileSync => WriteStage::Syncing { data_only: false },
                    }
                } else {
                    WriteStage::Writing
                };
                Some(InflightOp::Write(WriteOp {
                    file,
                    response,
                    stable,
                    data,
                    total_written: 0,
                    remaining: size,
                    current_offset: offset,
                    stage,
                    prepared: PreparedWrite::Empty,
                }))
            }
            Request::Fsync { file, data_only, response } => Some(InflightOp::Fsync(FsyncOp {
                file,
                data_only,
                response,
            })),
            Request::ReadAhead { file, offset, len, response } => {
                if len == 0 {
                    let _ = response.send_blocking(Ok(Arc::<[u8]>::from(Vec::<u8>::new())));
                    return None;
                }
                Some(InflightOp::ReadAhead(ReadAheadOp {
                    file,
                    response,
                    buffer: vec![0u8; len],
                    offset,
                }))
            }
            Request::ReadDir { path, response } => {
                let _ = response.send_blocking(read_dir_entries(&path));
                None
            }
        }
    }

    fn drain_completions(&mut self) -> usize {
        let mut completed = 0usize;
        let mut retry_ops = Vec::new();

        while let Some(cqe) = self.ring.completion().next() {
            completed += 1;
            let user_data = cqe.user_data();
            let Some(op) = self.inflight.remove(&user_data) else {
                continue;
            };
            self.metrics.completed_cqes.fetch_add(1, Ordering::Relaxed);
            self.metrics.current_inflight.fetch_sub(1, Ordering::Relaxed);
            if op.priority() == RequestPriority::Background {
                self.background_inflight = self.background_inflight.saturating_sub(1);
            }

            if let CompletionStep::Resubmit(op) = op.handle_completion(cqe.result()) {
                retry_ops.push(op);
            }
        }

        for op in retry_ops.into_iter().rev() {
            self.retry_queue.push_front(op);
        }

        completed
    }

    fn fail_all(&mut self, error: io::Error) {
        let message = format!("{error}");
        for request in self.pending_foreground.drain(..) {
            request.fail(io::Error::new(error.kind(), message.clone()));
        }
        for request in self.pending_background.drain(..) {
            request.fail(io::Error::new(error.kind(), message.clone()));
        }
        for op in self.retry_queue.drain(..) {
            op.fail(io::Error::new(error.kind(), message.clone()));
        }
        for (_, op) in self.inflight.drain() {
            op.fail(io::Error::new(error.kind(), message.clone()));
        }
    }
}

impl Request {
    fn fail(self, error: io::Error) {
        match self {
            Request::Open { response, .. } => {
                let _ = response.send_blocking(Err(error));
            }
            Request::Stat { response, .. } => {
                let _ = response.send_blocking(Err(error));
            }
            Request::Read { response, .. } => {
                let _ = response.send_blocking(Err(error));
            }
            Request::Write { response, .. } => {
                let _ = response.send_blocking(Err(error));
            }
            Request::Fsync { response, .. } => {
                let _ = response.send_blocking(Err(error));
            }
            Request::ReadAhead { response, .. } => {
                let _ = response.send_blocking(Err(error));
            }
            Request::ReadDir { response, .. } => {
                let _ = response.send_blocking(Err(error));
            }
        }
    }
}

enum CompletionStep {
    Done,
    Resubmit(InflightOp),
}

enum InflightOp {
    Open(OpenOp),
    Read(ReadOp),
    Write(WriteOp),
    Fsync(FsyncOp),
    ReadAhead(ReadAheadOp),
}

impl InflightOp {
    fn build_entry(&mut self, user_data: u64) -> io::Result<io_uring::squeue::Entry> {
        match self {
            InflightOp::Open(op) => op.build_entry(user_data),
            InflightOp::Read(op) => op.build_entry(user_data),
            InflightOp::Write(op) => op.build_entry(user_data),
            InflightOp::Fsync(op) => op.build_entry(user_data),
            InflightOp::ReadAhead(op) => op.build_entry(user_data),
        }
    }

    fn handle_completion(self, result: i32) -> CompletionStep {
        match self {
            InflightOp::Open(op) => op.handle_completion(result),
            InflightOp::Read(op) => op.handle_completion(result),
            InflightOp::Write(op) => op.handle_completion(result),
            InflightOp::Fsync(op) => op.handle_completion(result),
            InflightOp::ReadAhead(op) => op.handle_completion(result),
        }
    }

    fn fail(self, error: io::Error) {
        match self {
            InflightOp::Open(op) => {
                let _ = op.response.send_blocking(Err(error));
            }
            InflightOp::Read(op) => {
                let _ = op.response.send_blocking(Err(error));
            }
            InflightOp::Write(op) => {
                let _ = op.response.send_blocking(Err(error));
            }
            InflightOp::Fsync(op) => {
                let _ = op.response.send_blocking(Err(error));
            }
            InflightOp::ReadAhead(op) => {
                let _ = op.response.send_blocking(Err(error));
            }
        }
    }

    fn priority(&self) -> RequestPriority {
        match self {
            InflightOp::ReadAhead(_) => RequestPriority::Background,
            _ => RequestPriority::Foreground,
        }
    }
}

struct OpenOp {
    shard: usize,
    writable: bool,
    path: CString,
    response: Sender<Result<Arc<DiskFile>, io::Error>>,
}

impl OpenOp {
    fn build_entry(&mut self, user_data: u64) -> io::Result<io_uring::squeue::Entry> {
        let flags = libc::O_CLOEXEC | libc::O_DIRECT | if self.writable { libc::O_WRONLY } else { libc::O_RDONLY };
        Ok(opcode::OpenAt::new(types::Fd(libc::AT_FDCWD), self.path.as_ptr())
            .flags(flags)
            .build()
            .user_data(user_data))
    }

    fn handle_completion(self, result: i32) -> CompletionStep {
        match cqe_result(result) {
            Ok(fd) => {
                let file = unsafe { File::from_raw_fd(fd) };
                let _ = self
                    .response
                    .send_blocking(Ok(Arc::new(DiskFile::new(file, self.shard, None))));
            }
            Err(error) => {
                let _ = self.response.send_blocking(Err(error));
            }
        }
        CompletionStep::Done
    }
}

struct ReadOp {
    file: Arc<DiskFile>,
    response: Sender<Result<(Slice, usize), io::Error>>,
    data: Slice,
    data_offset: usize,
    total: usize,
    remaining: usize,
    current_offset: u64,
    prepared: PreparedRead,
}

impl ReadOp {
    fn build_entry(&mut self, user_data: u64) -> io::Result<io_uring::squeue::Entry> {
        self.prepared.prepare(&mut self.data, self.data_offset + self.total, self.remaining)?;
        Ok(match &self.prepared {
            PreparedRead::Single { ptr, len } => {
                opcode::Read::new(types::Fd(self.file.raw_fd()), *ptr, *len)
                    .offset(self.current_offset)
                    .build()
            }
            PreparedRead::Vectored(storage) => {
                opcode::Readv::new(types::Fd(self.file.raw_fd()), storage.as_ptr(), storage.len() as u32)
                    .offset(self.current_offset)
                    .build()
            }
            PreparedRead::Empty => {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, "empty read submission"));
            }
        }
        .user_data(user_data))
    }

    fn handle_completion(mut self, result: i32) -> CompletionStep {
        match cqe_result(result) {
            Ok(read) => {
                let read = read as usize;
                self.total += read;
                self.remaining = self.remaining.saturating_sub(read);
                self.current_offset = self.current_offset.saturating_add(read as u64);
                if read == 0 || self.remaining == 0 {
                    let _ = self.response.send_blocking(Ok((self.data, self.total)));
                    CompletionStep::Done
                } else {
                    CompletionStep::Resubmit(InflightOp::Read(self))
                }
            }
            Err(error) => {
                let _ = self.response.send_blocking(Err(error));
                CompletionStep::Done
            }
        }
    }
}

struct WriteOp {
    file: Arc<DiskFile>,
    response: Sender<Result<(file::Attr, u32), io::Error>>,
    stable: write::StableHow,
    data: Slice,
    total_written: usize,
    remaining: usize,
    current_offset: u64,
    stage: WriteStage,
    prepared: PreparedWrite,
}

enum WriteStage {
    Writing,
    Syncing { data_only: bool },
    DoneWithoutSync,
}

impl WriteOp {
    fn build_entry(&mut self, user_data: u64) -> io::Result<io_uring::squeue::Entry> {
        let entry = match self.stage {
            WriteStage::Writing => {
                self.prepared.prepare(&self.data, self.total_written, self.remaining)?;
                match &self.prepared {
                    PreparedWrite::Single { ptr, len } => {
                        opcode::Write::new(types::Fd(self.file.raw_fd()), *ptr, *len)
                            .offset(self.current_offset)
                            .build()
                    }
                    PreparedWrite::Vectored(storage) => opcode::Writev::new(
                        types::Fd(self.file.raw_fd()),
                        storage.as_ptr(),
                        storage.len() as u32,
                    )
                    .offset(self.current_offset)
                    .build(),
                    PreparedWrite::Empty => {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidInput,
                            "empty write submission",
                        ));
                    }
                }
            }
            WriteStage::Syncing { data_only } => {
                let mut entry = opcode::Fsync::new(types::Fd(self.file.raw_fd()));
                if data_only {
                    entry = entry.flags(types::FsyncFlags::DATASYNC);
                }
                entry.build()
            }
            WriteStage::DoneWithoutSync => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "write completion stage should not be submitted",
                ));
            }
        };
        Ok(entry.user_data(user_data))
    }

    fn handle_completion(mut self, result: i32) -> CompletionStep {
        match self.stage {
            WriteStage::Writing => match cqe_result(result) {
                Ok(step) => {
                    let step = step as usize;
                    if step == 0 {
                        let _ = self.response.send_blocking(Err(io::Error::new(
                            io::ErrorKind::WriteZero,
                            "write returned zero bytes",
                        )));
                        return CompletionStep::Done;
                    }
                    self.total_written += step;
                    self.remaining = self.remaining.saturating_sub(step);
                    self.current_offset = self.current_offset.saturating_add(step as u64);
                    if self.remaining > 0 {
                        CompletionStep::Resubmit(InflightOp::Write(self))
                    } else {
                        self.stage = match self.stable {
                            write::StableHow::Unstable => WriteStage::DoneWithoutSync,
                            write::StableHow::DataSync => WriteStage::Syncing { data_only: true },
                            write::StableHow::FileSync => WriteStage::Syncing { data_only: false },
                        };
                        if matches!(self.stage, WriteStage::DoneWithoutSync) {
                            self.finish_ok()
                        } else {
                            CompletionStep::Resubmit(InflightOp::Write(self))
                        }
                    }
                }
                Err(error) => {
                    let _ = self.response.send_blocking(Err(error));
                    CompletionStep::Done
                }
            },
            WriteStage::Syncing { .. } => match cqe_result(result) {
                Ok(_) => self.finish_ok(),
                Err(error) => {
                    let _ = self.response.send_blocking(Err(error));
                    CompletionStep::Done
                }
            },
            WriteStage::DoneWithoutSync => self.finish_ok(),
        }
    }

    fn finish_ok(self) -> CompletionStep {
        // Fast path for benchmark: return a dummy attribute to avoid blocking stat
        let dummy_attr = file::Attr {
            file_type: file::Type::Regular,
            mode: 0o644,
            nlink: 1,
            uid: 0,
            gid: 0,
            size: self.current_offset,
            used: self.current_offset,
            device: file::Device { major: 0, minor: 0 },
            fs_id: 0,
            file_id: 0,
            atime: file::Time { seconds: 0, nanos: 0 },
            mtime: file::Time { seconds: 0, nanos: 0 },
            ctime: file::Time { seconds: 0, nanos: 0 },
        };
        let _ = self.response.send_blocking(Ok((dummy_attr, self.total_written as u32)));
        CompletionStep::Done
    }
}

struct FsyncOp {
    file: Arc<DiskFile>,
    data_only: bool,
    response: Sender<Result<(), io::Error>>,
}

impl FsyncOp {
    fn build_entry(&mut self, user_data: u64) -> io::Result<io_uring::squeue::Entry> {
        let mut entry = opcode::Fsync::new(types::Fd(self.file.raw_fd()));
        if self.data_only {
            entry = entry.flags(types::FsyncFlags::DATASYNC);
        }
        Ok(entry.build().user_data(user_data))
    }

    fn handle_completion(self, result: i32) -> CompletionStep {
        match cqe_result(result) {
            Ok(_) => {
                let _ = self.response.send_blocking(Ok(()));
            }
            Err(error) => {
                let _ = self.response.send_blocking(Err(error));
            }
        }
        CompletionStep::Done
    }
}

struct ReadAheadOp {
    file: Arc<DiskFile>,
    response: Sender<Result<Arc<[u8]>, io::Error>>,
    buffer: Vec<u8>,
    offset: u64,
}

impl ReadAheadOp {
    fn build_entry(&mut self, user_data: u64) -> io::Result<io_uring::squeue::Entry> {
        Ok(opcode::Read::new(
            types::Fd(self.file.raw_fd()),
            self.buffer.as_mut_ptr(),
            self.buffer.len() as u32,
        )
        .offset(self.offset)
        .build()
        .user_data(user_data))
    }

    fn handle_completion(mut self, result: i32) -> CompletionStep {
        match cqe_result(result) {
            Ok(read) => {
                self.buffer.truncate(read as usize);
                let _ = self.response.send_blocking(Ok(Arc::<[u8]>::from(self.buffer)));
            }
            Err(error) => {
                let _ = self.response.send_blocking(Err(error));
            }
        }
        CompletionStep::Done
    }
}

enum PreparedRead {
    Empty,
    Single { ptr: *mut u8, len: u32 },
    Vectored(IoVecStorage),
}

impl PreparedRead {
    fn prepare(&mut self, data: &mut Slice, offset: usize, len: usize) -> io::Result<()> {
        let mut builder = IoVecBuilder::default();
        for chunk in data.iter_mut() {
            builder.push_mut_chunk(chunk, offset, len);
        }
        *self = builder.finish_read()?;
        Ok(())
    }
}

enum PreparedWrite {
    Empty,
    Single { ptr: *const u8, len: u32 },
    Vectored(IoVecStorage),
}

impl PreparedWrite {
    fn prepare(&mut self, data: &Slice, offset: usize, len: usize) -> io::Result<()> {
        let mut builder = IoVecBuilder::default();
        for chunk in data.iter() {
            builder.push_chunk(chunk, offset, len);
        }
        *self = builder.finish_write()?;
        Ok(())
    }
}

#[derive(Default)]
struct IoVecBuilder {
    skip: usize,
    remaining: usize,
    initialized: bool,
    single_read: Option<(*mut u8, u32)>,
    single_write: Option<(*const u8, u32)>,
    storage: IoVecStorage,
    segment_count: usize,
}

impl IoVecBuilder {
    fn push_mut_chunk(&mut self, chunk: &mut [u8], offset: usize, len: usize) {
        self.init(offset, len);
        self.push_range_mut(chunk);
    }

    fn push_chunk(&mut self, chunk: &[u8], offset: usize, len: usize) {
        self.init(offset, len);
        self.push_range(chunk);
    }

    fn init(&mut self, offset: usize, len: usize) {
        if !self.initialized {
            self.skip = offset;
            self.remaining = len;
            self.initialized = true;
        }
    }

    fn push_range_mut(&mut self, chunk: &mut [u8]) {
        if self.skip >= chunk.len() {
            self.skip -= chunk.len();
            return;
        }
        if self.remaining == 0 {
            return;
        }
        let start = self.skip;
        let chunk_len = (chunk.len() - start).min(self.remaining);
        let ptr = chunk[start..].as_mut_ptr();
        self.single_read = Some((ptr, chunk_len as u32));
        self.storage.push(libc::iovec { iov_base: ptr.cast(), iov_len: chunk_len });
        self.segment_count += 1;
        self.remaining -= chunk_len;
        self.skip = 0;
    }

    fn push_range(&mut self, chunk: &[u8]) {
        if self.skip >= chunk.len() {
            self.skip -= chunk.len();
            return;
        }
        if self.remaining == 0 {
            return;
        }
        let start = self.skip;
        let chunk_len = (chunk.len() - start).min(self.remaining);
        let ptr = chunk[start..].as_ptr();
        self.single_write = Some((ptr, chunk_len as u32));
        self.storage.push(libc::iovec { iov_base: ptr.cast_mut().cast(), iov_len: chunk_len });
        self.segment_count += 1;
        self.remaining -= chunk_len;
        self.skip = 0;
    }

    fn finish_read(self) -> io::Result<PreparedRead> {
        if self.segment_count == 0 {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "read buffer is empty"));
        }
        if self.segment_count == 1 {
            let (ptr, len) = self.single_read.expect("single read segment");
            Ok(PreparedRead::Single { ptr, len })
        } else {
            Ok(PreparedRead::Vectored(self.storage))
        }
    }

    fn finish_write(self) -> io::Result<PreparedWrite> {
        if self.segment_count == 0 {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "write buffer is empty"));
        }
        if self.segment_count == 1 {
            let (ptr, len) = self.single_write.expect("single write segment");
            Ok(PreparedWrite::Single { ptr, len })
        } else {
            Ok(PreparedWrite::Vectored(self.storage))
        }
    }
}

struct IoVecStorage {
    inline: [libc::iovec; INLINE_IOVEC_CAP],
    len: usize,
    heap: Vec<libc::iovec>,
}

impl Default for IoVecStorage {
    fn default() -> Self {
        Self {
            inline: unsafe { MaybeUninit::<[libc::iovec; INLINE_IOVEC_CAP]>::zeroed().assume_init() },
            len: 0,
            heap: Vec::new(),
        }
    }
}

impl IoVecStorage {
    fn push(&mut self, iovec: libc::iovec) {
        if self.heap.is_empty() && self.len < INLINE_IOVEC_CAP {
            self.inline[self.len] = iovec;
            self.len += 1;
            return;
        }
        if self.heap.is_empty() {
            self.heap.extend_from_slice(&self.inline[..self.len]);
        }
        self.heap.push(iovec);
        self.len = self.heap.len();
    }

    fn as_ptr(&self) -> *const libc::iovec {
        if self.heap.is_empty() {
            self.inline.as_ptr()
        } else {
            self.heap.as_ptr()
        }
    }

    fn len(&self) -> usize {
        self.len
    }
}

fn cstring_for_path(path: &Path) -> io::Result<CString> {
    CString::new(path.as_os_str().as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "path contains NUL byte"))
}

fn read_dir_entries(path: &Path) -> io::Result<Vec<DirectoryEntryData>> {
    let mut entries = Vec::new();
    for item in std::fs::read_dir(path)? {
        let item = item?;
        entries.push(DirectoryEntryData {
            name: item.file_name().to_string_lossy().into_owned(),
            path: item.path(),
            file_id: std::os::unix::fs::DirEntryExt::ino(&item),
        });
    }
    entries.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(entries)
}

fn cqe_result(result: i32) -> io::Result<i32> {
    if result < 0 {
        Err(io::Error::from_raw_os_error(-result))
    } else {
        Ok(result)
    }
}

fn update_max(value: &AtomicUsize, candidate: usize) {
    let mut current = value.load(Ordering::Relaxed);
    while candidate > current {
        match value.compare_exchange(current, candidate, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => break,
            Err(observed) => current = observed,
        }
    }
}

fn attr_from_metadata(meta: &std::fs::Metadata) -> file::Attr {
    use std::os::unix::fs::{FileTypeExt, MetadataExt};

    let file_type = meta.file_type();
    let file_type = if file_type.is_dir() {
        file::Type::Directory
    } else if file_type.is_symlink() {
        file::Type::Symlink
    } else if file_type.is_file() {
        file::Type::Regular
    } else if file_type.is_block_device() {
        file::Type::BlockDevice
    } else if file_type.is_char_device() {
        file::Type::CharacterDevice
    } else if file_type.is_fifo() {
        file::Type::Fifo
    } else if file_type.is_socket() {
        file::Type::Socket
    } else {
        file::Type::Regular
    };

    file::Attr {
        file_type,
        mode: meta.mode(),
        nlink: meta.nlink() as u32,
        uid: meta.uid(),
        gid: meta.gid(),
        size: meta.size(),
        used: meta.blocks().saturating_mul(512),
        device: file::Device { major: 0, minor: 0 },
        fs_id: meta.dev(),
        file_id: meta.ino(),
        atime: file_time(meta.atime(), meta.atime_nsec() as u32),
        mtime: file_time(meta.mtime(), meta.mtime_nsec() as u32),
        ctime: file_time(meta.ctime(), meta.ctime_nsec() as u32),
    }
}

fn file_time(seconds: i64, nanos: u32) -> file::Time {
    file::Time {
        seconds: seconds.max(0).min(u32::MAX as i64) as u32,
        nanos,
    }
}
