#!/usr/bin/env python3
"""Per-operation cost breakdown: server/client CPU, read/write syscalls and context
switches per completed request, plus per-thread CPU of the server."""
import os
import subprocess
import sys
import time

import run
from run import Server, parse_table, CLK

CONFIGS = [
    ("main", [], 128), ("branch", ["--max-in-flight", "32"], 128),
    ("main", [], 1024), ("branch", [], 1024), ("branch", ["--max-in-flight", "1048576"], 1024),
]


def threads(pid):
    out = {}
    for tid in os.listdir(f"/proc/{pid}/task"):
        try:
            with open(f"/proc/{pid}/task/{tid}/stat") as f:
                rest = f.read()
            name = rest[rest.index("(") + 1:rest.rindex(")")]
            parts = rest.rsplit(")", 1)[1].split()
            cpu = int(parts[11]) + int(parts[12])
            with open(f"/proc/{pid}/task/{tid}/status") as f:
                st = dict(l.split(":", 1) for l in f if ":" in l)
            vol = int(st["voluntary_ctxt_switches"])
            nonvol = int(st["nonvoluntary_ctxt_switches"])
            out[tid] = (name, cpu, vol, nonvol)
        except FileNotFoundError:
            pass
    return out


def io(pid):
    with open(f"/proc/{pid}/io") as f:
        d = dict(l.split(":") for l in f)
    return int(d["syscr"]), int(d["syscw"])


def probe(name, sargs, depth, secs=6):
    srv = Server(name, sargs)
    try:
        cmd = run.client_cmd(srv.port, ["--op", "getattr", "--connections", "1", "--depth", str(depth),
                                        "--duration", f"{secs}s", "--warmup", "1s"])
        cli = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        time.sleep(1.5)  # past the warm-up
        t0 = time.time()
        s_thr0, s_io0 = threads(srv.pid), io(srv.pid)
        c_thr0, c_io0 = threads(cli.pid), io(cli.pid)
        time.sleep(3.0)
        t1 = time.time()
        s_thr1, s_io1 = threads(srv.pid), io(srv.pid)
        c_thr1, c_io1 = threads(cli.pid), io(cli.pid)
        out, _ = cli.communicate(timeout=60)
    finally:
        srv.stop()
    ops_rate = next(iter(parse_table(out).values()))["ops"]
    ops = ops_rate * (t1 - t0)

    def per_op(a, b):
        cpu = sum(b[t][1] - a[t][1] for t in b if t in a) / CLK
        vol = sum(b[t][2] - a[t][2] for t in b if t in a)
        nonvol = sum(b[t][3] - a[t][3] for t in b if t in a)
        return cpu / ops * 1e6, vol / ops, nonvol / ops

    s_cpu, s_vol, s_nv = per_op(s_thr0, s_thr1)
    c_cpu, c_vol, c_nv = per_op(c_thr0, c_thr1)
    busiest = sorted(((s_thr1[t][1] - s_thr0[t][1]) / CLK / (t1 - t0) * 100, s_thr1[t][0])
                     for t in s_thr1 if t in s_thr0)[-3:]
    label = name + ("" if not sargs else " " + " ".join(sargs))
    print(f"{label:<30} depth {depth:>5}: {ops_rate:>8.0f} ops/s | server {s_cpu:5.2f} us CPU/op, "
          f"read sys/op {(s_io1[0] - s_io0[0]) / ops:5.3f}, write sys/op {(s_io1[1] - s_io0[1]) / ops:5.3f}, "
          f"ctxsw/op {s_vol:5.3f}+{s_nv:5.3f} | client {c_cpu:5.2f} us CPU/op, "
          f"read sys/op {(c_io1[0] - c_io0[0]) / ops:5.3f}, write sys/op {(c_io1[1] - c_io0[1]) / ops:5.3f} | "
          f"busiest server threads {[(round(p), n) for p, n in busiest]}", flush=True)


if __name__ == "__main__":
    reps = int(sys.argv[1]) if len(sys.argv) > 1 else 2
    for _ in range(reps):
        for name, sargs, depth in CONFIGS:
            probe(name, sargs, depth)
