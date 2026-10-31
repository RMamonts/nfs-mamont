#!/usr/bin/env python3
"""End-to-end benchmark runner: main vs vyar/in_flight_limit.

Server pinned to CPUs 0-1, load generator to CPUs 2-3. Every configuration is run
REPEATS times; the median is reported. Results are appended to results.jsonl.
"""
import json
import os
import signal
import socket
import statistics
import subprocess
import sys
import threading
import time

BENCH = "/home/user/bench"
SERVERS = {
    "main": f"{BENCH}/server-main",
    "branch": f"{BENCH}/server-branch",
}
CLIENT = f"{BENCH}/nfs-bench"
SLOW = f"{BENCH}/slow-reader"
SERVER_CPUS = os.environ.get("SERVER_CPUS", "0,1")
CLIENT_CPUS = os.environ.get("CLIENT_CPUS", "2,3")
CLK = os.sysconf("SC_CLK_TCK")
PORT = 20490
RESULTS = f"{BENCH}/results.jsonl"


def cpu_ticks(pid):
    with open(f"/proc/{pid}/stat") as f:
        parts = f.read().rsplit(")", 1)[1].split()
    return int(parts[11]) + int(parts[12])  # utime + stime


def io_counts(pid):
    out = {}
    with open(f"/proc/{pid}/io") as f:
        for line in f:
            k, v = line.split(":")
            out[k] = int(v)
    return out


def rss_kib(pid):
    out = {}
    with open(f"/proc/{pid}/status") as f:
        for line in f:
            if line.startswith(("VmRSS", "VmHWM")):
                k, v = line.split(":")
                out[k] = int(v.split()[0])
    return out


class Server:
    def __init__(self, name, args):
        global PORT
        PORT += 1
        self.port = PORT
        binary = SERVERS["main" if name == "main" else "branch"]
        cmd = ["taskset", "-c", SERVER_CPUS, binary, "--bind", f"127.0.0.1:{self.port}"] + args
        self.proc = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        deadline = time.time() + 10
        while True:
            try:
                socket.create_connection(("127.0.0.1", self.port), timeout=0.2).close()
                break
            except OSError:
                if time.time() > deadline or self.proc.poll() is not None:
                    raise RuntimeError("server did not start: " + self.proc.stdout.read().decode())
                time.sleep(0.05)

    @property
    def pid(self):
        return self.proc.pid

    def stop(self):
        self.proc.send_signal(signal.SIGKILL)
        self.proc.wait()


def parse_table(text):
    rows = {}
    lines = text.splitlines()
    for i, line in enumerate(lines):
        if line.startswith("op ") and "ops/s" in line:
            for row in lines[i + 1:]:
                cols = row.split()
                if len(cols) != 9:
                    break
                rows[cols[0]] = {
                    "ops": float(cols[1]), "mibs": float(cols[2]), "avg": cols[3],
                    "p50": cols[4], "p99": cols[5], "p999": cols[6], "max": cols[7],
                    "errors": int(cols[8]),
                }
    return rows


def to_us(text):
    for unit, mult in (("ms", 1e3), ("us", 1.0), ("s", 1e6)):
        if text.endswith(unit):
            return float(text[: -len(unit)]) * mult
    return float(text)


def client_cmd(port, args):
    return ["taskset", "-c", CLIENT_CPUS, CLIENT, "--target", f"127.0.0.1:{port}", "--quiet"] + args


def bench_once(server_name, server_args, client_args):
    srv = Server(server_name, server_args)
    try:
        t0, c0, io0 = time.time(), cpu_ticks(srv.pid), io_counts(srv.pid)
        out = subprocess.run(client_cmd(srv.port, client_args), capture_output=True, text=True, timeout=600)
        t1, c1, io1 = time.time(), cpu_ticks(srv.pid), io_counts(srv.pid)
        mem = rss_kib(srv.pid)
        mem["syscr"] = io1["syscr"] - io0["syscr"]
        mem["syscw"] = io1["syscw"] - io0["syscw"]
        mem["wall"] = t1 - t0
    finally:
        srv.stop()
    rows = parse_table(out.stdout)
    if not rows:
        raise RuntimeError(f"no result table: {out.stdout}\n{out.stderr}")
    cpu = (c1 - c0) / CLK / (t1 - t0) * 100
    return rows, cpu, mem


def bench(label, server_name, server_args, client_args, repeats=3):
    runs = [bench_once(server_name, server_args, client_args) for _ in range(repeats)]
    op = next(iter(runs[0][0]))
    ops = [r[0][op]["ops"] for r in runs]
    med_i = sorted(range(len(ops)), key=lambda i: ops[i])[len(ops) // 2]
    med = runs[med_i][0][op]
    rec = {
        "label": label, "server": server_name, "server_args": server_args,
        "client_args": client_args, "op": op,
        "ops_median": statistics.median(ops), "ops_all": ops,
        "mibs": med["mibs"], "avg_us": to_us(med["avg"]), "p50_us": to_us(med["p50"]),
        "p99_us": to_us(med["p99"]), "p999_us": to_us(med["p999"]),
        "errors": med["errors"],
        "server_cpu_pct": round(runs[med_i][1], 1),
        "server_rss_kib": runs[med_i][2].get("VmRSS"),
        "server_hwm_kib": runs[med_i][2].get("VmHWM"),
        "syscr": runs[med_i][2].get("syscr"), "syscw": runs[med_i][2].get("syscw"),
        "wall": runs[med_i][2].get("wall"),
    }
    with open(RESULTS, "a") as f:
        f.write(json.dumps(rec) + "\n")
    spread = (max(ops) - min(ops)) / rec["ops_median"] * 100 if rec["ops_median"] else 0
    print(
        f"{label:<28} {server_name + ' ' + ' '.join(server_args):<34} {' '.join(client_args):<48} "
        f"{rec['ops_median']:>9.0f} ops/s ±{spread:4.1f}%  {rec['mibs']:>7.1f} MiB/s  "
        f"p50 {rec['p50_us']:>8.0f}us p99 {rec['p99_us']:>8.0f}us  cpu {rec['server_cpu_pct']:>5.1f}%  "
        f"hwm {rec['server_hwm_kib'] / 1024:>6.1f}MiB  err {rec['errors']}",
        flush=True,
    )
    return rec


def variants(limits):
    out = [("main", [])]
    for lim in limits:
        out.append(("branch", [] if lim == "default" else ["--max-in-flight", str(lim)]))
    return out


def e1_cpu_bound_single_conn():
    for depth in [1, 8, 32, 128, 256, 512, 1024]:
        for name, sargs in variants(["default", 1048576, 32]):
            bench("E1 getattr c1", name, sargs,
                  ["--op", "getattr", "--connections", "1", "--depth", str(depth),
                   "--duration", "6s", "--warmup", "1s"])


def e2_cpu_bound_multi_conn():
    for depth in [64, 512]:
        for name, sargs in variants(["default", 1048576]):
            bench("E2 getattr c4", name, sargs,
                  ["--op", "getattr", "--connections", "4", "--depth", str(depth),
                   "--duration", "6s", "--warmup", "1s"])


def e3_latency_bound():
    # default pool (64 workers) with 2 ms backend: the pool caps at 32k ops/s
    for depth in [256, 1024]:
        for name, sargs in variants(["default"]):
            bench("E3a lat2ms conc64", name, sargs + ["--latency", "2ms"],
                  ["--op", "getattr", "--connections", "1", "--depth", str(depth),
                   "--duration", "6s", "--warmup", "1s"])
    # deep backend: 1024 workers, 20 ms -> pool caps at 51k ops/s
    for depth in [256, 512, 1024]:
        for name, sargs in variants(["default", 1024]):
            bench("E3b lat20ms conc1024", name, sargs + ["--latency", "20ms", "--concurrency", "1024"],
                  ["--op", "getattr", "--connections", "1", "--depth", str(depth),
                   "--duration", "8s", "--warmup", "2s"])


def e4_data_path():
    for op, conns, depth in [("read:64k", 1, 32), ("read:1m", 1, 16), ("write:64k", 1, 32),
                             ("read:4k", 4, 64)]:
        for name, sargs in variants(["default"]):
            bench("E4 data", name, sargs,
                  ["--op", op, "--connections", str(conns), "--depth", str(depth),
                   "--duration", "6s", "--warmup", "1s"])


if __name__ == "__main__":
    which = sys.argv[1:] or ["e1", "e2", "e3", "e4"]
    for w in which:
        print(f"===== {w} =====", flush=True)
        globals()[{"e1": "e1_cpu_bound_single_conn", "e2": "e2_cpu_bound_multi_conn",
                   "e3": "e3_latency_bound", "e4": "e4_data_path"}[w]]()
