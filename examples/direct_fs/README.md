# directfs prototype

`directfs` is a performance-oriented prototype VFS implementation derived from `mirrorfs`.

Goals:
- improve throughput/latency on large-block fio workloads
- prioritize fast-path sequential and random I/O over strict safety checks
- keep metadata compatibility with existing mirrorfs behavior

## Design choices

- Read/write max NFS transfer size increased to 1 MiB.
- Read/write allocator sizing in the example server is tuned for deep queues.
- Data path uses blocking `pread`/`pwrite` syscalls with an optional `O_DIRECT` open path.
- Write path assumes no concurrent writes to the same file for best performance.
- On direct-I/O failure, it falls back to buffered I/O.

## Run

```bash
cargo run --example directfs -- /path/to/export 0.0.0.0:2049
```

Enable/disable direct I/O mode:

```bash
# default is enabled
NFS_DIRECTFS_ODIRECT=1 cargo run --example directfs -- /path/to/export 0.0.0.0:2049

# disable direct I/O fallback mode
NFS_DIRECTFS_ODIRECT=0 cargo run --example directfs -- /path/to/export 0.0.0.0:2049
```

## fio profiles for comparison with NFS Ganesha

Use both block sizes (`1M`, `128K`) and `iodepth=32`.

### read

```bash
fio --name=read_bs1m --rw=read --bs=1M --iodepth=32 --ioengine=libaio --direct=1 --size=8G --numjobs=1 --filename=/mnt/nfs/testfile
fio --name=read_bs128k --rw=read --bs=128K --iodepth=32 --ioengine=libaio --direct=1 --size=8G --numjobs=1 --filename=/mnt/nfs/testfile
```

### write (single writer per file)

```bash
fio --name=write_bs1m --rw=write --bs=1M --iodepth=32 --ioengine=libaio --direct=1 --size=8G --numjobs=1 --filename=/mnt/nfs/testfile
fio --name=write_bs128k --rw=write --bs=128K --iodepth=32 --ioengine=libaio --direct=1 --size=8G --numjobs=1 --filename=/mnt/nfs/testfile
```

### randrw

```bash
fio --name=randrw_bs1m --rw=randrw --rwmixread=70 --bs=1M --iodepth=32 --ioengine=libaio --direct=1 --size=8G --numjobs=1 --filename=/mnt/nfs/testfile
fio --name=randrw_bs128k --rw=randrw --rwmixread=70 --bs=128K --iodepth=32 --ioengine=libaio --direct=1 --size=8G --numjobs=1 --filename=/mnt/nfs/testfile
```

## Prototype caveats

- Not intended as a correctness-first filesystem implementation.
- Blocking syscalls are used intentionally in the data path for this prototype.
- Best results are expected with aligned offsets/sizes and one writer per file.
