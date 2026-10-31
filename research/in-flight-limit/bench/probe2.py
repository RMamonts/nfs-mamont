#!/usr/bin/env python3
"""Read-side counters per request (instrumented servers)."""
import re
import subprocess
import sys
import time

import run

run.SERVERS = {"main": f"{run.BENCH}/server-main-probe", "branch": f"{run.BENCH}/server-branch-probe"}

CONFIGS = [
    ("main", [], 128), ("branch", ["--max-in-flight", "32"], 128),
    ("main", [], 1024), ("branch", [], 1024), ("branch", ["--max-in-flight", "1048576"], 1024),
]
LINE = re.compile(r"probe t=([\d.]+) polls=(\d+) pending=(\d+) ready=(\d+) bytes=(\d+) msgs=(\d+) parks=(\d+)")


def probe(name, sargs, depth):
    srv = run.Server(name, sargs)
    try:
        cmd = run.client_cmd(srv.port, ["--op", "getattr", "--connections", "1", "--depth", str(depth),
                                        "--duration", "6s", "--warmup", "1s"])
        out = subprocess.run(cmd, capture_output=True, text=True, timeout=60).stdout
    finally:
        srv.proc.kill()
        log = srv.proc.stdout.read().decode()
        srv.proc.wait()
    ops = next(iter(run.parse_table(out).values()))["ops"]
    samples = [tuple(float(x) for x in m.groups()) for m in LINE.finditer(log)]
    a, b = samples[2], samples[-2]  # skip start-up and the tail
    d = [y - x for x, y in zip(a, b)]
    _, polls, pending, ready, nbytes, msgs, parks = d
    label = name + ("" if not sargs else " " + " ".join(sargs))
    print(f"{label:<30} depth {depth:>5}: {ops:>8.0f} ops/s | per request: socket polls {polls / msgs:5.2f}, "
          f"waits for socket (Pending) {pending / msgs:5.3f}, read syscalls with data {ready / msgs:5.3f} "
          f"({nbytes / max(ready, 1):6.0f} B/read = {msgs / max(ready, 1):5.1f} req/read), "
          f"budget parks {parks / msgs:5.3f}", flush=True)


if __name__ == "__main__":
    for _ in range(int(sys.argv[1]) if len(sys.argv) > 1 else 1):
        for c in CONFIGS:
            probe(*c)
