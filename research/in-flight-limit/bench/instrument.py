#!/usr/bin/env python3
"""Adds read-side counters to a worktree (identical for main and the branch):
socket polls, polls that returned Pending (the read task had to wait for the socket),
successful reads, bytes, parsed messages and, on the branch, parks on the budget.
The mock server prints the counters once per second."""
import sys
import pathlib

wt = pathlib.Path(sys.argv[1])

lib = wt / "nfs-mamont/src/lib.rs"
s = lib.read_text()
if "pub mod probe" not in s:
    s += '''
#[doc(hidden)]
pub mod probe {
    use std::sync::atomic::AtomicU64;
    pub static READ_POLLS: AtomicU64 = AtomicU64::new(0);
    pub static READ_PENDING: AtomicU64 = AtomicU64::new(0);
    pub static READ_READY: AtomicU64 = AtomicU64::new(0);
    pub static READ_BYTES: AtomicU64 = AtomicU64::new(0);
    pub static MESSAGES: AtomicU64 = AtomicU64::new(0);
    pub static BUDGET_PARKS: AtomicU64 = AtomicU64::new(0);
}
'''
    lib.write_text(s)

read = wt / "nfs-mamont/src/task/connection/read.rs"
s = read.read_text()
if "struct Counting" not in s:
    s += '''
/// Counts socket polls for the probe.
struct Counting<R>(R);

impl<R: tokio::io::AsyncRead + Unpin> tokio::io::AsyncRead for Counting<R> {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        use std::sync::atomic::Ordering::Relaxed;
        crate::probe::READ_POLLS.fetch_add(1, Relaxed);
        let before = buf.filled().len();
        let result = std::pin::Pin::new(&mut self.0).poll_read(cx, buf);
        match &result {
            std::task::Poll::Pending => {
                crate::probe::READ_PENDING.fetch_add(1, Relaxed);
            }
            std::task::Poll::Ready(Ok(())) => {
                crate::probe::READ_READY.fetch_add(1, Relaxed);
                crate::probe::READ_BYTES.fetch_add((buf.filled().len() - before) as u64, Relaxed);
            }
            _ => {}
        }
        result
    }
}
'''
    s = s.replace("RpcParser::new(readhalf, allocator)", "RpcParser::new(Counting(readhalf), allocator)")
    s = s.replace("RpcParser::new(self.readhalf, self.allocator)",
                  "RpcParser::new(Counting(self.readhalf), self.allocator)")
    s = s.replace("            match parser.next_message().await {",
                  "            let message = parser.next_message().await;\n"
                  "            crate::probe::MESSAGES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);\n"
                  "            match message {")
    read.write_text(s)

limit = wt / "nfs-mamont/src/task/connection/limit.rs"
if limit.exists():
    s = limit.read_text()
    if "BUDGET_PARKS" not in s:
        s = s.replace("                self.limiter.notify.notified().await;",
                      "                crate::probe::BUDGET_PARKS.fetch_add(1, Ordering::Relaxed);\n"
                      "                self.limiter.notify.notified().await;")
        limit.write_text(s)

server = wt / "mock-vfs/src/bin/mock-nfs-server.rs"
s = server.read_text()
if "probe t=" not in s:
    s = s.replace("async fn main() -> ExitCode {", '''async fn main() -> ExitCode {
    std::thread::spawn(|| {
        use std::sync::atomic::Ordering::Relaxed;
        use nfs_mamont::probe::*;
        let start = std::time::Instant::now();
        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
            eprintln!(
                "probe t={:.3} polls={} pending={} ready={} bytes={} msgs={} parks={}",
                start.elapsed().as_secs_f64(),
                READ_POLLS.load(Relaxed),
                READ_PENDING.load(Relaxed),
                READ_READY.load(Relaxed),
                READ_BYTES.load(Relaxed),
                MESSAGES.load(Relaxed),
                BUDGET_PARKS.load(Relaxed),
            );
        }
    });''', 1)
    server.write_text(s)
print("instrumented", wt)
