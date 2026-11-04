#!/usr/bin/env python3
"""Turns data/results.jsonl into markdown tables, one per experiment.

Repeated runs of the same point (same label, server and arguments) are collapsed into
the median of every numeric field; the number of runs is shown.
"""
import json
import os
import statistics
import sys
from collections import OrderedDict

HERE = os.path.dirname(os.path.abspath(__file__))
path = sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, "..", "data", "results.jsonl")
recs = [json.loads(line) for line in open(path) if line.strip()]


def server_label(rec):
    args = rec.get("server_args", [])
    extra = " ".join(a for a in args if not a.startswith("--latency") and a not in ("1ms", "500us"))
    # keep only the flags that distinguish variants of the same experiment
    keep = []
    skip = 0
    for a in args:
        if skip:
            skip -= 1
            continue
        if a in ("--latency", "--bandwidth", "--buf-count"):
            skip = 1
            continue
        keep.append(a)
    extra = " ".join(keep)
    return rec["server"] + (f" {extra}" if extra else "")


def arg(rec, key, flag):
    a = rec.get(key, rec.get("client_args", []))
    return a[a.index(flag) + 1] if flag in a else "-"


def median_of(group, field):
    values = [r[field] for r in group if isinstance(r.get(field), (int, float))]
    return statistics.median(values) if values else None


def fmt(value, unit=""):
    if value is None:
        return "-"
    if unit == "us":
        return f"{value / 1000:.2f} ms" if value >= 1000 else f"{value:.0f} µs"
    if unit == "k":
        return f"{value / 1000:.1f}k"
    if unit == "mib":
        return f"{value / 1024:.1f}"
    return f"{value:.0f}"


def stat(group, key):
    values = [r.get("stats", {}).get(key) for r in group]
    values = [v for v in values if v is not None]
    return f"{statistics.median(values):.0f}" if values else "-"


def group_runs(records, key):
    groups = OrderedDict()
    for r in records:
        groups.setdefault(key(r), []).append(r)
    return groups


def table(header, rows):
    print("| " + " | ".join(header) + " |")
    print("|" + "---|" * len(header))
    for row in rows:
        print("| " + " | ".join(str(c) for c in row) + " |")
    print()


by_label = group_runs(recs, lambda r: r["label"])

for label, records in by_label.items():
    print(f"### {label}\n")
    if "ops_median" in records[0]:
        groups = group_runs(records, lambda r: (server_label(r), " ".join(r["client_args"])))
        rows = []
        for (server, cargs), g in groups.items():
            rows.append([
                server, arg(g[0], "client_args", "--op"), arg(g[0], "client_args", "--connections"),
                arg(g[0], "client_args", "--depth"), fmt(median_of(g, "ops_median"), "k"),
                fmt(median_of(g, "mibs")), fmt(median_of(g, "p50_us"), "us"), fmt(median_of(g, "p99_us"), "us"),
                fmt(median_of(g, "server_cpu_pct")) + " %", fmt(median_of(g, "server_hwm_kib"), "mib"),
                stat(g, "in_flight_waits"), stat(g, "pending_waits"), stat(g, "data_budget_waits"),
                stat(g, "allocator_waits"), stat(g, "sched_rotations"),
            ])
        table(["server", "op", "conns", "depth", "ops/s", "MiB/s", "p50", "p99", "server CPU", "RSS peak MiB",
               "in-flight waits", "pending waits", "data waits", "allocator waits", "DRR rotations"], rows)
    elif "accepted_requests" in records[0]:
        rows = []
        for r in records:
            timeline = " → ".join(str(s[1]) for s in r["rss_timeline_mib"][::4])
            rows.append([server_label(r), r["accepted_requests"], f"{r['rss_mib_final']:.1f}", f"{r['hwm_mib']:.1f}",
                         timeline, r.get("stats", {}).get("write_timeouts", "-")])
        table(["server", "accepted requests", "RSS final MiB", "RSS peak MiB", "RSS timeline MiB (every 4 s)", "write timeouts"], rows)
    elif "victim_alone" in records[0]:
        rows = []
        for r in records:
            alone, hog = r["victim_alone"], r["victim_with_hog"]
            rows.append([server_label(r), fmt(alone["ops"], "k"), fmt(hog["ops"], "k"),
                         fmt(hog.get("p50_us"), "us"), hog.get("note", ""),
                         r.get("stats", {}).get("data_budget_waits", "-"), r.get("stats", {}).get("conn_data_waits", "-"),
                         r.get("stats", {}).get("allocator_waits", "-")])
        table(["server", "victim alone ops/s", "victim with hog ops/s", "victim p50", "note", "data waits", "conn data waits", "allocator waits"], rows)
    elif "heavy_ops" in records[0]:
        groups = group_runs(records, lambda r: server_label(r))
        rows = []
        for server, g in groups.items():
            rows.append([
                server, len(g), fmt(median_of(g, "heavy_ops"), "k"), fmt(median_of(g, "heavy_mibs")),
                fmt(median_of(g, "heavy_p50_us"), "us"), fmt(median_of(g, "light_ops"), "k"), fmt(median_of(g, "light_mibs")),
                fmt(median_of(g, "light_p50_us"), "us"), fmt(median_of(g, "light_p99_us"), "us"),
                stat(g, "pending_waits"), stat(g, "sched_rotations"), stat(g, "conn_data_waits"),
            ])
        table(["server", "runs", "heavy ops/s", "heavy MiB/s", "heavy p50", "light ops/s", "light MiB/s", "light p50", "light p99",
               "pending waits", "DRR rotations", "conn data waits"], rows)
