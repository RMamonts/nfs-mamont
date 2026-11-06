/// Stage of an NFS request between the five timestamps of its pipeline:
/// received (T0) → parsed (T1) → picked by a VFS worker (T2) → backend done (T3) →
/// picked by the write task (T4) → reply written (T5).
///
/// Parsing (T0 → T1) and writing (T4 → T5) is covered by the per-message metrics instead.
#[derive(Clone, Copy)]
pub enum Stage {
    /// T1 → T2: waiting in the VFS pool queue.
    VfsQueue,
    /// T2 → T3: executing in the VFS worker, backend included.
    Vfs,
    /// T3 → T4: waiting in the connection reply queue.
    ReplyQueue,
}

#[cfg(feature = "metrics")]
impl Stage {
    pub(super) const ALL: [Stage; 3] = [Stage::VfsQueue, Stage::Vfs, Stage::ReplyQueue];

    pub(super) fn as_str(self) -> &'static str {
        match self {
            Stage::VfsQueue => "vfs_queue",
            Stage::Vfs => "vfs",
            Stage::ReplyQueue => "reply_queue",
        }
    }
}

/// Pipeline queue whose depth is tracked.
#[derive(Clone, Copy)]
pub enum Queue {
    /// Read tasks → VFS worker pool.
    Vfs,
    /// workers → connection write tasks, summed over connections.
    Reply,
}

#[cfg(feature = "metrics")]
impl Queue {
    pub(super) const ALL: [Queue; 2] = [Queue::Vfs, Queue::Reply];

    pub(super) fn as_str(self) -> &'static str {
        match self {
            Queue::Vfs => "vfs",
            Queue::Reply => "reply",
        }
    }
}

/// Kind of work instrumented with its own tokio task monitor.
#[derive(Clone, Copy)]
pub enum TaskKind {
    /// Connection read task, one per connection.
    Read,
    /// Connection write task, one per connection.
    Write,
}

#[cfg(feature = "metrics")]
impl TaskKind {
    pub(super) const ALL: [TaskKind; 2] = [TaskKind::Read, TaskKind::Write];

    pub(super) fn as_str(self) -> &'static str {
        match self {
            TaskKind::Read => "read",
            TaskKind::Write => "write",
        }
    }
}
