#!/usr/bin/env bash
# 2x2 control experiment: reuse order {FIFO, LIFO} x base alignment {page, +16 bytes},
# all with the same prototype pool code; upstream64k as the reference.
set -u
BIN="$(dirname "$0")/../target/release/e2e"
REPS=${1:-5}
SECS=${2:-2.5}
run() { for _ in $(seq "$REPS"); do "$BIN" --secs "$SECS" "$@" 2>/dev/null | grep '^alloc='; done; }
for spec in "read mem" "read pread" "write mem"; do
  set -- $spec
  for alloc in upstream64k fifo64k+16 fifo64k lifo64k+16 fixed64k lockfree64k; do
    run --alloc "$alloc" --op "$1" --size 65536 --mode "$2"
  done
done
