#!/usr/bin/env python3
"""Back-pressure experiments: memory under a client that never reads (E5), allocator
starvation of a well-behaved client (E6), latency of a light client next to a heavy
one (E7)."""
import json
import subprocess
import sys
import threading
import time

import run
from run import Server, rss_kib, cpu_ticks, parse_table, to_us, client_cmd, SLOW, CLIENT_CPUS

VARIANTS = [("main", []), ("branch", []), ("branch", ["--max-in-flight", "32"])]


def label(name, sargs):
    return name + ("" if not sargs else " " + " ".join(sargs))


def write(rec):
    with open(run.RESULTS, "a") as f:
        f.write(json.dumps(rec) + "\n")


def e5_memory():
    for op in ["getattr", "null"]:
        for name, sargs in VARIANTS:
            srv = Server(name, sargs)
            samples = []
            try:
                proc = subprocess.Popen(
                    ["taskset", "-c", CLIENT_CPUS, SLOW, "--target", f"127.0.0.1:{srv.port}",
                     "--op", op, "--duration", "15s"],
                    stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
                start = time.time()
                while proc.poll() is None:
                    time.sleep(1)
                    m = rss_kib(srv.pid)
                    samples.append((round(time.time() - start, 1), m["VmRSS"] // 1024))
                out = proc.stdout.read()
                total = int(out.strip().splitlines()[-1].split("=")[1])
                mem = rss_kib(srv.pid)
            finally:
                srv.stop()
            rec = {"label": "E5 slow-reader " + op, "server": label(name, sargs),
                   "accepted_requests": total, "rss_mib_final": mem["VmRSS"] / 1024,
                   "hwm_mib": mem["VmHWM"] / 1024, "rss_timeline_mib": samples}
            write(rec)
            print(f"E5 {op:<8} {label(name, sargs):<30} accepted {total:>10}  "
                  f"RSS final {mem['VmRSS'] / 1024:>8.1f} MiB  HWM {mem['VmHWM'] / 1024:>8.1f} MiB  "
                  f"timeline {[s[1] for s in samples][::3]}", flush=True)


def victim(port, client_args, timeout):
    try:
        out = subprocess.run(client_cmd(port, client_args), capture_output=True, text=True,
                             timeout=timeout)
        rows = parse_table(out.stdout)
        if not rows:
            return {"ops": 0.0, "note": out.stdout[-300:] + out.stderr[-300:]}
        row = next(iter(rows.values()))
        return {"ops": row["ops"], "p50_us": to_us(row["p50"]), "p99_us": to_us(row["p99"]),
                "errors": row["errors"]}
    except subprocess.TimeoutExpired:
        return {"ops": 0.0, "note": f"stalled: no result within {timeout}s"}


def e6_allocator_starvation():
    victim_args = ["--op", "read:4k", "--connections", "1", "--depth", "16",
                   "--duration", "6s", "--warmup", "1s"]
    for name, sargs in VARIANTS:
        # baseline without the hog
        srv = Server(name, sargs)
        try:
            base = victim(srv.port, victim_args, 30)
        finally:
            srv.stop()
        srv = Server(name, sargs)
        try:
            hog = subprocess.Popen(
                ["taskset", "-c", CLIENT_CPUS, SLOW, "--target", f"127.0.0.1:{srv.port}",
                 "--op", "read", "--block", "1M", "--duration", "25s"],
                stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
            time.sleep(3)
            with_hog = victim(srv.port, victim_args, 15)
            hog.kill()
            hog.wait()
        finally:
            srv.stop()
        rec = {"label": "E6 allocator hog", "server": label(name, sargs),
               "victim_alone": base, "victim_with_hog": with_hog}
        write(rec)
        print(f"E6 {label(name, sargs):<30} victim read:4k alone {base['ops']:>8.0f} ops/s, "
              f"next to a READ-1M hog {with_hog['ops']:>8.0f} ops/s {with_hog.get('note', '')}",
              flush=True)


def e7_fairness():
    variants = [("main", []), ("branch", []), ("branch", ["--max-in-flight", "64"])]
    for name, sargs in variants:
        srv = Server(name, sargs + ["--latency", "1ms"])
        result = {}
        try:
            def heavy():
                out = subprocess.run(
                    ["taskset", "-c", "2", run.CLIENT, "--target", f"127.0.0.1:{srv.port}",
                     "--quiet", "--op", "getattr", "--connections", "1", "--depth", "2048",
                     "--duration", "14s", "--warmup", "1s"],
                    capture_output=True, text=True, timeout=120)
                result["heavy"] = next(iter(parse_table(out.stdout).values()))

            t = threading.Thread(target=heavy)
            t.start()
            time.sleep(3)
            out = subprocess.run(
                ["taskset", "-c", "3", run.CLIENT, "--target", f"127.0.0.1:{srv.port}", "--quiet",
                 "--op", "getattr", "--connections", "1", "--depth", "1", "--duration", "8s",
                 "--warmup", "1s"],
                capture_output=True, text=True, timeout=120)
            light = next(iter(parse_table(out.stdout).values()))
            t.join()
        finally:
            srv.stop()
        heavy_row = result["heavy"]
        rec = {"label": "E7 fairness lat1ms", "server": label(name, sargs),
               "heavy_ops": heavy_row["ops"], "heavy_p50_us": to_us(heavy_row["p50"]),
               "light_ops": light["ops"], "light_p50_us": to_us(light["p50"]),
               "light_p99_us": to_us(light["p99"])}
        write(rec)
        print(f"E7 {label(name, sargs):<30} heavy {heavy_row['ops']:>8.0f} ops/s (p50 {heavy_row['p50']}), "
              f"light depth-1 client: {light['ops']:>6.0f} ops/s p50 {light['p50']} p99 {light['p99']}",
              flush=True)


if __name__ == "__main__":
    for w in sys.argv[1:] or ["e5", "e6", "e7"]:
        print(f"===== {w} =====", flush=True)
        {"e5": e5_memory, "e6": e6_allocator_starvation, "e7": e7_fairness}[w]()
