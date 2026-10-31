#!/usr/bin/env python3
"""Turns results.jsonl into markdown tables."""
import json
import sys
from collections import OrderedDict

path = sys.argv[1] if len(sys.argv) > 1 else "/home/user/bench/results.jsonl"
recs = [json.loads(l) for l in open(path) if l.strip()]


def server_label(r):
    s = r["server"]
    a = [x for x in r.get("server_args", []) if x not in ("--latency", "--concurrency")]
    args = r.get("server_args", [])
    lim = "256 (default)" if s == "branch" else "—"
    if "--max-in-flight" in args:
        lim = args[args.index("--max-in-flight") + 1]
        if lim == "1048576":
            lim = "1M (no limit)"
    return f"{s} / {lim}"


def depth(r):
    a = r["client_args"]
    return int(a[a.index("--depth") + 1])


def conns(r):
    a = r["client_args"]
    return int(a[a.index("--connections") + 1])


groups = OrderedDict()
for r in recs:
    if "ops_median" not in r:
        continue
    groups.setdefault(r["label"], []).append(r)

for label, rows in groups.items():
    print(f"\n### {label}\n")
    print("| server / max_in_flight | op | conns | depth | ops/s (median of 3) | runs | p50 | p99 | server CPU | RSS peak |")
    print("|---|---|---|---|---|---|---|---|---|---|")
    for r in rows:
        runs = ", ".join(f"{x / 1000:.1f}k" for x in r["ops_all"])
        print(
            f"| {server_label(r)} | {r['op']} | {conns(r)} | {depth(r)} | {r['ops_median']:,.0f} | {runs} | "
            f"{r['p50_us']:,.0f} µs | {r['p99_us']:,.0f} µs | {r['server_cpu_pct']:.0f}% | "
            f"{r['server_hwm_kib'] / 1024:.1f} MiB |"
        )

for r in recs:
    if r.get("label", "").startswith(("E5", "E6", "E7")):
        print(json.dumps(r))
