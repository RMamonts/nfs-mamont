use std::collections::HashMap;
use std::io;
use std::os::unix::io::RawFd;
use std::sync::Arc;
use std::thread;

use io_uring::{opcode, types, IoUring};
use tokio::sync::{mpsc, oneshot};

#[derive(Debug)]
pub struct UringExecutor {
    sender: mpsc::UnboundedSender<UringRequest>,
}

impl UringExecutor {
    pub fn new(entries: u32) -> Option<Arc<Self>> {
        let ring = IoUring::new(entries).ok()?;
        let (sender, receiver) = mpsc::unbounded_channel();
        let executor = Arc::new(Self { sender });

        let _ = thread::Builder::new()
            .name("mirrorfs-uring".to_string())
            .spawn(move || run_uring(ring, receiver));

        Some(executor)
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
}

enum UringRequest {
    Fsync {
        fd: RawFd,
        flags: types::FsyncFlags,
        reply: oneshot::Sender<io::Result<()>>,
    },
    Write {
        fd: RawFd,
        offset: u64,
        buffer: Vec<u8>,
        reply: oneshot::Sender<io::Result<usize>>,
    },
    Read {
        fd: RawFd,
        offset: u64,
        len: usize,
        reply: oneshot::Sender<io::Result<Vec<u8>>>,
    },
}

enum InFlight {
    Fsync(oneshot::Sender<io::Result<()>>),
    Write {
        reply: oneshot::Sender<io::Result<usize>>,
        buffer: Vec<u8>,
    },
    Read {
        reply: oneshot::Sender<io::Result<Vec<u8>>>,
        buffer: Vec<u8>,
    },
}

fn run_uring(mut ring: IoUring, mut receiver: mpsc::UnboundedReceiver<UringRequest>) {
    let mut next_id: u64 = 1;

    loop {
        let Some(request) = receiver.blocking_recv() else {
            break;
        };

        let mut batch = vec![request];
        while let Ok(request) = receiver.try_recv() {
            batch.push(request);
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
                            .user_data(user_data);

                        if unsafe { submission.push(&entry) }.is_err() {
                            let _ = reply.send(Err(io::Error::new(
                                io::ErrorKind::Other,
                                "io_uring submission queue full",
                            )));
                            continue;
                        }

                        inflight.insert(user_data, InFlight::Fsync(reply));
                        submitted += 1;
                    }
                    UringRequest::Write { fd, offset, buffer, reply } => {
                        let user_data = next_id;
                        next_id = next_id.wrapping_add(1);
                        let entry = opcode::Write::new(
                            types::Fd(fd),
                            buffer.as_ptr(),
                            buffer.len() as u32,
                        )
                        .offset(offset)
                        .build()
                        .user_data(user_data);

                        if unsafe { submission.push(&entry) }.is_err() {
                            let _ = reply.send(Err(io::Error::new(
                                io::ErrorKind::Other,
                                "io_uring submission queue full",
                            )));
                            continue;
                        }

                        inflight.insert(user_data, InFlight::Write { reply, buffer });
                        submitted += 1;
                    }
                    UringRequest::Read { fd, offset, len, reply } => {
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
                        .user_data(user_data);

                        if unsafe { submission.push(&entry) }.is_err() {
                            let _ = reply.send(Err(io::Error::new(
                                io::ErrorKind::Other,
                                "io_uring submission queue full",
                            )));
                            continue;
                        }

                        inflight.insert(user_data, InFlight::Read { reply, buffer });
                        submitted += 1;
                    }
                }
            }
        }

        if submitted == 0 {
            continue;
        }

        if let Err(error) = ring.submit_and_wait(1) {
            fail_inflight(inflight, error);
            continue;
        }

        while !inflight.is_empty() {
            {
                let mut completions = ring.completion();
                while let Some(cqe) = completions.next() {
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
                        InFlight::Write { reply, buffer } => {
                            let max = buffer.len();
                            let result = result.and_then(|bytes| {
                                if bytes > max {
                                    Err(io::Error::new(io::ErrorKind::InvalidData, "write overflow"))
                                } else {
                                    Ok(bytes)
                                }
                            });
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
                    }
                }
            }

            if inflight.is_empty() {
                break;
            }

            if let Err(error) = ring.submit_and_wait(1) {
                fail_inflight(inflight, error);
                break;
            }
        }
    }
}

fn fail_inflight(inflight: HashMap<u64, InFlight>, error: io::Error) {
    let error = Arc::new(error);
    for (_, request) in inflight {
        match request {
            InFlight::Fsync(reply) => {
                let _ = reply.send(Err(io::Error::new(error.kind(), error.to_string())));
            }
            InFlight::Write { reply, .. } => {
                let _ = reply.send(Err(io::Error::new(error.kind(), error.to_string())));
            }
            InFlight::Read { reply, .. } => {
                let _ = reply.send(Err(io::Error::new(error.kind(), error.to_string())));
            }
        }
    }
}
