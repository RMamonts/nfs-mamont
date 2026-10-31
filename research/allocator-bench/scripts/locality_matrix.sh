#!/usr/bin/env bash
# FIFO (upstream) vs LIFO (prototypes): memcpy throughput and RSS growth.
# Every configuration runs in a fresh process so RSS numbers are not mixed up.
set -u
BIN="$(dirname "$0")/../target/release/locality"
for ring in 1 32; do
  # mirror-fs default: 64 KiB x 2048 = 128 MiB
  "$BIN" --alloc upstream --block 65536 --count 2048 --size 65536 --ring $ring
  "$BIN" --alloc lockfree --block 65536 --count 2048 --size 65536 --ring $ring
  "$BIN" --alloc buddy    --block 65536 --count 2048 --size 65536 --ring $ring
  # mirror-fs config.toml: 1 MiB x 2048 = 2 GiB, clients send <= 64 KiB (rtmax)
  "$BIN" --alloc upstream --block 1048576 --count 2048 --size 65536 --ring $ring
  "$BIN" --alloc buddy    --block 1048576 --count 2048 --size 65536 --ring $ring
  # pool larger than this VM's 260 MiB L3: 64 KiB x 8192 = 512 MiB
  "$BIN" --alloc upstream --block 65536 --count 8192 --size 65536 --ring $ring
  "$BIN" --alloc lockfree --block 65536 --count 8192 --size 65536 --ring $ring
  "$BIN" --alloc lockfree --block 65536 --count 8192 --size 65536 --ring $ring --huge true
  # 1 MiB requests in a 2 GiB pool (large rsize)
  "$BIN" --alloc upstream --block 1048576 --count 2048 --size 1048576 --ring $ring
  "$BIN" --alloc lockfree --block 1048576 --count 2048 --size 1048576 --ring $ring
  "$BIN" --alloc lockfree --block 1048576 --count 2048 --size 1048576 --ring $ring --huge true
done
# 2x2 controls (reuse order x alignment) and THP for FIFO
for ring in 1 32; do
  "$BIN" --alloc fifo --block 65536 --count 2048 --size 65536 --ring $ring --misalign 0
  "$BIN" --alloc fifo --block 65536 --count 2048 --size 65536 --ring $ring --misalign 16
  "$BIN" --alloc lifo --block 65536 --count 2048 --size 65536 --ring $ring --misalign 0
  "$BIN" --alloc lifo --block 65536 --count 2048 --size 65536 --ring $ring --misalign 16
  "$BIN" --alloc fifo --block 65536 --count 8192 --size 65536 --ring $ring --misalign 0
  "$BIN" --alloc fifo --block 65536 --count 8192 --size 65536 --ring $ring --misalign 0 --huge true
done
