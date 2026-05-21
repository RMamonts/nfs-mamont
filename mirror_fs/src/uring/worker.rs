use std::collections::HashMap;
use std::io;
use std::os::unix::io::RawFd;
use std::sync::Arc;
use std::thread;
use std::time::Instant;

use io_uring::{opcode, types};

use crate::uring::buffer::{FixedBufferPool, MAX_BATCH, BATCH_WAIT};
use crate::uring::helpers::statx_to_data;
use crate::uring::types::{InFlight, UringRequest};

pub fn run_uring(
    mut ring: io_uring::IoUring,
    mut receiver: tokio::sync::mpsc::UnboundedReceiver<UringRequest>,
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
                            .user_data(user_data);

                        if unsafe { submission.push(&entry) }.is_err() {
                            let _ =
                                reply.send(Err(io::Error::other("io_uring submission queue full")));
                            continue;
                        }

                        inflight.insert(user_data, InFlight::Fsync(reply));
                        submitted += 1;
                    }
                    UringRequest::Write { fd, offset, buffer, reply } => {
                        if let Some(pool) = pool.as_mut() {
                            if buffer.len() <= pool.len() {
                                if let Some(index) = pool.take() {
                                    pool.buffer_mut(index)[..buffer.len()].copy_from_slice(&buffer);
                                    let user_data = next_id;
                                    next_id = next_id.wrapping_add(1);
                                    let entry = opcode::WriteFixed::new(
                                        types::Fd(fd),
                                        pool.buffer(index).as_ptr(),
                                        buffer.len() as u32,
                                        index as u16,
                                    )
                                    .offset(offset)
                                    .build()
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
                                        InFlight::WriteFixed { reply, index, len: buffer.len() },
                                    );
                                    submitted += 1;
                                    continue;
                                }
                            }
                        }

                        let user_data = next_id;
                        next_id = next_id.wrapping_add(1);
                        let entry =
                            opcode::Write::new(types::Fd(fd), buffer.as_ptr(), buffer.len() as u32)
                                .offset(offset)
                                .build()
                                .user_data(user_data);

                        if unsafe { submission.push(&entry) }.is_err() {
                            let _ =
                                reply.send(Err(io::Error::other("io_uring submission queue full")));
                            continue;
                        }

                        inflight.insert(user_data, InFlight::Write { reply, buffer });
                        submitted += 1;
                    }
                    UringRequest::Read { fd, offset, len, reply } => {
                        if let Some(pool) = pool.as_mut() {
                            if len <= pool.len() {
                                if let Some(index) = pool.take() {
                                    let user_data = next_id;
                                    next_id = next_id.wrapping_add(1);
                                    let entry = opcode::ReadFixed::new(
                                        types::Fd(fd),
                                        pool.buffer_mut_ref(index),
                                        len as u32,
                                        index as u16,
                                    )
                                    .offset(offset)
                                    .build()
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
                        InFlight::Write { reply, buffer } => {
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
                            let _ = reply.send(result);
                        }
                        InFlight::WriteFixed { reply, index, len } => {
                            let result = result.and_then(|bytes| {
                                if bytes > len {
                                    Err(io::Error::new(
                                        io::ErrorKind::InvalidData,
                                        "write overflow",
                                    ))
                                } else {
                                    Ok(bytes)
                                }
                            });
                            if let Some(pool) = pool.as_mut() {
                                pool.release(index);
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
                                    data.copy_from_slice(&pool.buffer(index)[..bytes]);
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

pub fn fail_inflight(
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
            InFlight::Write { reply, .. } => {
                let _ = reply.send(Err(io::Error::new(error.kind(), error.to_string())));
            }
            InFlight::WriteFixed { reply, index, .. } => {
                if let Some(pool) = pool.as_mut() {
                    pool.release(index);
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

