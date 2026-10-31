#!/usr/bin/env python3
"""Interleaved A/B/C/D runs (5 rounds) to separate real differences from VM noise."""
import json, statistics, subprocess, time, sys
import run

BIN = {"main": "server-main", "branch": "server-branch", "raii": "server-raii", "batch": "server-batch"}

class S(run.Server):
    def __init__(self, name, args):
        run.SERVERS["branch"] = f"{run.BENCH}/{BIN[name]}"
        run.SERVERS["main"] = f"{run.BENCH}/server-main"
        super().__init__("main" if name == "main" else "branch", args)

CASES = [
    ("K1 4conn depth64 (below limit)", ["--op", "getattr", "--connections", "4", "--depth", "64"]),
    ("K2 4conn depth512 (above limit)", ["--op", "getattr", "--connections", "4", "--depth", "512"]),
    ("K3 1conn depth1024 (above limit)", ["--op", "getattr", "--connections", "1", "--depth", "1024"]),
]
ROUNDS = int(sys.argv[1]) if len(sys.argv) > 1 else 5
res = {}
for case, cargs in CASES:
    for r in range(ROUNDS):
        for name in ["main", "branch", "raii", "batch"]:
            srv = S(name, [])
            try:
                c0, t0 = run.cpu_ticks(srv.pid), time.time()
                out = subprocess.run(run.client_cmd(srv.port, cargs + ["--duration", "6s", "--warmup", "1s"]),
                                     capture_output=True, text=True, timeout=120).stdout
                c1, t1 = run.cpu_ticks(srv.pid), time.time()
            finally:
                srv.stop()
            row = next(iter(run.parse_table(out).values()))
            cpu_us_per_op = (c1 - c0) / run.CLK / (t1 - t0) / row["ops"] * 1e6
            res.setdefault((case, name), []).append((row["ops"], run.to_us(row["p50"]), run.to_us(row["p99"]), cpu_us_per_op))
    for name in ["main", "branch", "raii", "batch"]:
        v = res[(case, name)]
        ops = [x[0] for x in v]
        print(f"{case:<34} {name:<7} ops/s median {statistics.median(ops):>9.0f}  (min {min(ops):>8.0f} max {max(ops):>8.0f})  "
              f"p50 {statistics.median(x[1] for x in v):>7.0f}us  p99 {statistics.median(x[2] for x in v):>7.0f}us  "
              f"server CPU/op {statistics.median(x[3] for x in v):5.2f}us", flush=True)
        with open(run.RESULTS, "a") as f:
            f.write(json.dumps({"label": "confirm " + case, "server": name, "runs": v}) + "\n")
