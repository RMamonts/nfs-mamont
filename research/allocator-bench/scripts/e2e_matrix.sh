#!/usr/bin/env bash
# Runs the end-to-end matrix; every configuration is a separate process.
# Usage: scripts/e2e_matrix.sh [reps] [secs] > results/e2e.txt
set -u
BIN="$(dirname "$0")/../target/release/e2e"
REPS=${1:-3}
SECS=${2:-2.5}
run() { for _ in $(seq "$REPS"); do "$BIN" --secs "$SECS" "$@" 2>/dev/null | grep '^alloc='; done; }
for op in read write; do
  for mode in mem pread preadv blocking; do
    for alloc in upstream64k upstream1m upstream4k lockfree64k lockfree4k buddy; do
      run --alloc "$alloc" --op "$op" --size 65536 --mode "$mode"
    done
  done
  for alloc in upstream64k upstream1m upstream4k lockfree4k buddy; do
    run --alloc "$alloc" --op "$op" --size 4096 --mode mem
  done
done
