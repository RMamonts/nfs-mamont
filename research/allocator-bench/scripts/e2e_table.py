#!/usr/bin/env python3
"""Aggregates e2e_matrix.sh output: median ops/s and server CPU us/op per config."""
import re
import statistics
import sys
from collections import OrderedDict

rows = OrderedDict()
pat = re.compile(r"(\w[\w/%]*)=\s*([\w.+]+)")
for line in open(sys.argv[1]):
    if not line.startswith("alloc="):
        continue
    kv = dict(pat.findall(line))
    key = (kv["op"], kv["size"], kv["mode"], kv["alloc"])
    rows.setdefault(key, []).append(
        (float(kv["ops/s"]), float(kv["server_cpu_us/op"]), int(kv["bad"]))
    )

base = {}
for (op, size, mode, alloc), v in rows.items():
    if alloc == "upstream64k":
        base[(op, size, mode)] = statistics.median(x[1] for x in v)

print(f"{'op':<6}{'size':>7} {'mode':<9}{'allocator':<13}{'ops/s':>9}{'cpu us/op':>11}{'vs up64k':>10}  bad")
last = None
for (op, size, mode, alloc), v in rows.items():
    if last and last != (op, size, mode):
        print()
    last = (op, size, mode)
    ops = statistics.median(x[0] for x in v)
    cpu = statistics.median(x[1] for x in v)
    bad = sum(x[2] for x in v)
    b = base.get((op, size, mode))
    rel = f"{(cpu / b - 1) * 100:+.1f}%" if b else ""
    print(f"{op:<6}{size:>7} {mode:<9}{alloc:<13}{ops:>9.0f}{cpu:>11.2f}{rel:>10}  {bad}")
