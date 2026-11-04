#!/usr/bin/env python3
"""Experiments for research/qos-limits: global budgets, DRR scheduling and the
allocator as the last line of defence.

Two server binaries are compared: `server-main` (the `main` branch, no limits) and
`server-exp` (this branch) in several configurations. The server is pinned to
SERVER_CPUS, the load generators to CLIENT_CPUS. Every point is run REPEATS times and
the median is reported. Records are appended to data/results.jsonl.

Usage: bench.py [e1 e2 e4 e5 e6 e7 e8 e9 e10] (default: all)
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

HERE = os.path.dirname(os.path.abspath(__file__))
BIN = os.environ.get("BENCH_BIN", os.path.join(HERE, "bin"))
DATA = os.environ.get("BENCH_DATA", os.path.join(HERE, "..", "data"))
SERVER_MAIN = os.path.join(BIN, "server-main")
SERVER_EXP = os.path.join(BIN, "server-exp")
CLIENT = os.path.join(BIN, "nfs-bench")
SLOW = os.path.join(BIN, "slow-reader")
SERVER_CPUS = os.environ.get("SERVER_CPUS", "0,1")
CLIENT_CPUS = os.environ.get("CLIENT_CPUS", "2,3")
REPEATS = int(os.environ.get("REPEATS", "3"))
CLK = os.sysconf("SC_CLK_TCK")
RESULTS = os.path.join(DATA, "results.jsonl")
PORT = [20490]

# Server variants: name -> (binary, flags). The pool is 64 x 1 MiB unless a variant
# says otherwise; "auto" lets the server derive the budget from the pool/workers.
VARIANTS = {
    "main": (SERVER_MAIN, []),
    "inflight": (SERVER_EXP, ["--max-in-flight", "256"]),
    "budgets": (SERVER_EXP, ["--max-in-flight", "256", "--max-pending", "auto",
                             "--max-connections", "4096", "--data-budget", "auto",
                             "--max-data-per-conn", "32M", "--write-timeout", "30s"]),
    "drr": (SERVER_EXP, ["--max-in-flight", "256", "--max-pending", "auto",
                         "--max-connections", "4096", "--data-budget", "auto",
                         "--max-data-per-conn", "32M", "--write-timeout", "30s",
                         "--scheduler", "drr"]),
    # one mechanism at a time, to price each of them
    "pending": (SERVER_EXP, ["--max-in-flight", "256", "--max-pending", "auto"]),
    "wtimeout": (SERVER_EXP, ["--max-in-flight", "256", "--write-timeout", "30s"]),
    "databudget": (SERVER_EXP, ["--max-in-flight", "256", "--data-budget", "auto",
                                "--max-data-per-conn", "32M"]),
    "drr-only": (SERVER_EXP, ["--max-in-flight", "256", "--scheduler", "drr"]),
}


def variant(name, extra=()):
    binary, flags = VARIANTS[name]
    return binary, list(flags) + list(extra)


def cpu_ticks(pid):
    with open(f"/proc/{pid}/stat") as f:
        parts = f.read().rsplit(")", 1)[1].split()
    return int(parts[11]) + int(parts[12])  # utime + stime


def rss_kib(pid):
    out = {}
    with open(f"/proc/{pid}/status") as f:
        for line in f:
            if line.startswith(("VmRSS", "VmHWM")):
                k, v = line.split(":")
                out[k] = int(v.split()[0])
    return out


class Server:
    """A server process; `stop()` sends SIGINT so the binary prints its final stats."""

    def __init__(self, name, extra=()):
        PORT[0] += 1
        self.port = PORT[0]
        self.name = name
        binary, flags = variant(name, extra)
        cmd = ["taskset", "-c", SERVER_CPUS, binary, "--bind", f"127.0.0.1:{self.port}"] + flags
        self.proc = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
        self.output = []
        self.reader = threading.Thread(target=self._read, daemon=True)
        self.reader.start()
        deadline = time.time() + 15
        while True:
            try:
                socket.create_connection(("127.0.0.1", self.port), timeout=0.2).close()
                break
            except OSError:
                if time.time() > deadline or self.proc.poll() is not None:
                    raise RuntimeError(f"server {name} did not start: {''.join(self.output)}")
                time.sleep(0.05)

    def _read(self):
        for line in self.proc.stdout:
            self.output.append(line)

    @property
    def pid(self):
        return self.proc.pid

    def stop(self):
        """Stops the server and returns its final stats line, if it printed one."""
        try:
            self.proc.send_signal(signal.SIGINT)
            self.proc.wait(timeout=10)
        except (subprocess.TimeoutExpired, ProcessLookupError):
            self.proc.kill()
            self.proc.wait()
        self.reader.join(timeout=5)
        stats = [l for l in self.output if l.startswith("stats:")]
        return parse_stats(stats[-1]) if stats else {}


def parse_stats(line):
    out = {}
    for item in line.split(":", 1)[1].split():
        if "=" in item:
            k, v = item.split("=", 1)
            try:
                out[k] = float(v) if "." in v else int(v)
            except ValueError:
                out[k] = v
    return out


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


def client_cmd(port, args, cpus=CLIENT_CPUS):
    return ["taskset", "-c", cpus, CLIENT, "--target", f"127.0.0.1:{port}", "--quiet"] + args


def run_client(port, args, timeout=600, cpus=CLIENT_CPUS):
    out = subprocess.run(client_cmd(port, args, cpus), capture_output=True, text=True, timeout=timeout)
    rows = parse_table(out.stdout)
    if not rows:
        raise RuntimeError(f"no result table: {out.stdout}\n{out.stderr}")
    return next(iter(rows.values()))


def write(rec):
    os.makedirs(DATA, exist_ok=True)
    with open(RESULTS, "a") as f:
        f.write(json.dumps(rec) + "\n")


def bench_once(name, server_extra, client_args):
    srv = Server(name, server_extra)
    try:
        t0, c0 = time.time(), cpu_ticks(srv.pid)
        out = subprocess.run(client_cmd(srv.port, client_args), capture_output=True, text=True, timeout=600)
        t1, c1 = time.time(), cpu_ticks(srv.pid)
        mem = rss_kib(srv.pid)
    finally:
        stats = srv.stop()
    rows = parse_table(out.stdout)
    if not rows:
        raise RuntimeError(f"no result table: {out.stdout}\n{out.stderr}")
    cpu = (c1 - c0) / CLK / (t1 - t0) * 100
    return next(iter(rows.values())), cpu, mem, stats


def bench(label, name, server_extra, client_args, repeats=REPEATS):
    runs = [bench_once(name, server_extra, client_args) for _ in range(repeats)]
    ops = [r[0]["ops"] for r in runs]
    med_i = sorted(range(len(ops)), key=lambda i: ops[i])[len(ops) // 2]
    row, cpu, mem, stats = runs[med_i]
    rec = {
        "label": label, "server": name, "server_args": list(server_extra),
        "client_args": client_args,
        "ops_median": statistics.median(ops), "ops_all": ops,
        "mibs": row["mibs"], "avg_us": to_us(row["avg"]), "p50_us": to_us(row["p50"]),
        "p99_us": to_us(row["p99"]), "p999_us": to_us(row["p999"]), "errors": row["errors"],
        "server_cpu_pct": round(cpu, 1), "server_rss_kib": mem.get("VmRSS"),
        "server_hwm_kib": mem.get("VmHWM"), "stats": stats,
    }
    write(rec)
    spread = (max(ops) - min(ops)) / rec["ops_median"] * 100 if rec["ops_median"] else 0
    print(
        f"{label:<22} {name:<9} {' '.join(client_args):<52} "
        f"{rec['ops_median']:>9.0f} ops/s ±{spread:4.1f}%  {rec['mibs']:>7.1f} MiB/s  "
        f"p50 {rec['p50_us']:>8.0f}us p99 {rec['p99_us']:>8.0f}us  cpu {cpu:>5.1f}%  "
        f"hwm {(mem.get('VmHWM') or 0) / 1024:>6.1f}MiB  err {row['errors']}  "
        f"{short_stats(stats)}",
        flush=True,
    )
    return rec


def short_stats(stats):
    keys = ["in_flight_waits", "pending_waits", "data_budget_waits", "conn_data_waits",
            "allocator_waits", "write_timeouts", "sched_rotations"]
    return " ".join(f"{k.replace('_waits', 'W')}={stats[k]}" for k in keys if stats.get(k))


ALL = ["main", "inflight", "budgets", "drr"]


def e1_single_connection():
    """Cost of the mechanisms: one connection, GETATTR, no backend latency."""
    for depth in [1, 32, 256, 1024]:
        for name in ALL:
            bench("E1 getattr c1", name, [],
                  ["--op", "getattr", "--connections", "1", "--depth", str(depth),
                   "--duration", "6s", "--warmup", "1s"])


def e2_four_connections():
    for depth in [64, 512]:
        for name in ALL:
            bench("E2 getattr c4", name, [],
                  ["--op", "getattr", "--connections", "4", "--depth", str(depth),
                   "--duration", "6s", "--warmup", "1s"])


def e4_data_path():
    for op, conns, depth in [("read:64k", 1, 32), ("read:1m", 1, 16), ("write:64k", 1, 32),
                             ("read:4k", 4, 64)]:
        for name in ALL:
            bench("E4 data", name, [],
                  ["--op", op, "--connections", str(conns), "--depth", str(depth),
                   "--duration", "6s", "--warmup", "1s"])


def slow_reader(port, args, duration):
    return subprocess.Popen(
        ["taskset", "-c", CLIENT_CPUS, SLOW, "--target", f"127.0.0.1:{port}",
         "--duration", duration] + args,
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)


def e5_non_reading_client():
    """Memory under a client that pipelines GETATTR and never reads; then the write
    timeout closing such a client."""
    for name, extra in [("main", []), ("inflight", []), ("budgets", []),
                        ("budgets", ["--write-timeout", "5s"])]:
        srv = Server(name, extra)
        samples = []
        try:
            proc = slow_reader(srv.port, ["--op", "getattr"], "15s")
            start = time.time()
            while proc.poll() is None:
                time.sleep(1)
                m = rss_kib(srv.pid)
                samples.append((round(time.time() - start, 1), m["VmRSS"] // 1024))
            out = proc.stdout.read()
            total = int(out.strip().splitlines()[-1].split("=")[1])
            mem = rss_kib(srv.pid)
        finally:
            stats = srv.stop()
        rec = {"label": "E5 non-reading getattr", "server": name, "server_args": extra,
               "accepted_requests": total, "rss_mib_final": mem["VmRSS"] / 1024,
               "hwm_mib": mem["VmHWM"] / 1024, "rss_timeline_mib": samples, "stats": stats}
        write(rec)
        print(f"E5 {name:<9} {' '.join(extra):<22} accepted {total:>10}  RSS {mem['VmRSS'] / 1024:>8.1f} MiB  "
              f"HWM {mem['VmHWM'] / 1024:>8.1f} MiB  timeline {[s[1] for s in samples][::3]}  "
              f"{short_stats(stats)}", flush=True)


def victim(port, args, timeout):
    try:
        row = run_client(port, args, timeout)
        return {"ops": row["ops"], "p50_us": to_us(row["p50"]), "p99_us": to_us(row["p99"]),
                "errors": row["errors"]}
    except subprocess.TimeoutExpired:
        return {"ops": 0.0, "note": f"stalled: no result within {timeout}s"}
    except RuntimeError as err:
        return {"ops": 0.0, "note": str(err)[-200:]}


def e6_allocator_hog():
    """A client sends READ 1 MiB and never reads: does a well-behaved READ 4k client
    next to it still get served?"""
    victim_args = ["--op", "read:4k", "--connections", "1", "--depth", "16",
                   "--duration", "6s", "--warmup", "1s"]
    for name in ALL:
        srv = Server(name)
        try:
            alone = victim(srv.port, victim_args, 30)
        finally:
            srv.stop()
        srv = Server(name)
        try:
            hog = slow_reader(srv.port, ["--op", "read", "--block", "1M"], "25s")
            time.sleep(3)
            with_hog = victim(srv.port, victim_args, 15)
            hog.kill()
            hog.wait()
        finally:
            stats = srv.stop()
        rec = {"label": "E6 allocator hog", "server": name, "victim_alone": alone,
               "victim_with_hog": with_hog, "stats": stats}
        write(rec)
        print(f"E6 {name:<9} victim read:4k alone {alone['ops']:>8.0f} ops/s, next to a READ-1M hog "
              f"{with_hog['ops']:>8.0f} ops/s {with_hog.get('note', '')}  {short_stats(stats)}", flush=True)


def two_clients(name, server_extra, heavy_args, light_args, heavy_cpu="2", light_cpu="3",
                light_delay=3, label=""):
    srv = Server(name, server_extra)
    result = {}
    try:
        def heavy():
            result["heavy"] = run_client(srv.port, heavy_args, 180, heavy_cpu)

        t = threading.Thread(target=heavy)
        t.start()
        time.sleep(light_delay)
        light = run_client(srv.port, light_args, 180, light_cpu)
        t.join()
    finally:
        stats = srv.stop()
    heavy_row = result["heavy"]
    rec = {"label": label, "server": name, "server_args": list(server_extra),
           "heavy_args": heavy_args, "light_args": light_args,
           "heavy_ops": heavy_row["ops"], "heavy_mibs": heavy_row["mibs"],
           "heavy_p50_us": to_us(heavy_row["p50"]), "heavy_p99_us": to_us(heavy_row["p99"]),
           "light_ops": light["ops"], "light_mibs": light["mibs"],
           "light_p50_us": to_us(light["p50"]), "light_p99_us": to_us(light["p99"]),
           "stats": stats}
    write(rec)
    print(f"{label} {name:<9} {' '.join(server_extra):<22} heavy {heavy_row['ops']:>8.0f} ops/s "
          f"{heavy_row['mibs']:>7.1f} MiB/s p50 {heavy_row['p50']:>8}  |  light {light['ops']:>7.0f} ops/s "
          f"{light['mibs']:>6.1f} MiB/s p50 {light['p50']:>8} p99 {light['p99']:>8}  {short_stats(stats)}",
          flush=True)
    return rec


def e7_heavy_light():
    """Latency of a depth-1 client next to a depth-2048 client, 1 ms backend."""
    heavy = ["--op", "getattr", "--connections", "1", "--depth", "2048", "--duration", "14s", "--warmup", "1s"]
    light = ["--op", "getattr", "--connections", "1", "--depth", "1", "--duration", "8s", "--warmup", "1s"]
    for name, extra in [("main", []), ("inflight", []), ("inflight", ["--max-in-flight", "64"]),
                        ("budgets", []), ("drr", [])]:
        for _ in range(REPEATS):
            two_clients(name, extra + ["--latency", "1ms"], heavy, light, label="E7 heavy+light")


def e8_many_connections():
    """Many busy connections: memory, throughput and the latency of a probe client."""
    for conns in [256, 1024]:
        heavy = ["--op", "getattr", "--connections", str(conns), "--depth", "16",
                 "--duration", "14s", "--warmup", "2s"]
        probe = ["--op", "getattr", "--connections", "1", "--depth", "1", "--duration", "8s", "--warmup", "1s"]
        for name in ALL:
            for _ in range(REPEATS):
                rec = two_clients(name, ["--latency", "1ms"], heavy, probe, label=f"E8 c{conns}+probe")


def e9_fast_slow():
    """A fast client (depth 256) next to a slow one (depth 2): no equal split."""
    fast = ["--op", "getattr", "--connections", "1", "--depth", "256", "--duration", "10s", "--warmup", "1s"]
    slow = ["--op", "getattr", "--connections", "1", "--depth", "2", "--duration", "8s", "--warmup", "1s"]
    for name in ALL:
        for _ in range(REPEATS):
            two_clients(name, ["--latency", "1ms"], fast, slow, light_delay=1, label="E9 fast+slow")


def e10_cost_weighted():
    """Metadata client next to a 1 MiB streaming client on a bandwidth-bound backend:
    FIFO serves requests, DRR serves cost."""
    meta = ["--op", "getattr", "--connections", "1", "--depth", "64", "--duration", "8s", "--warmup", "1s"]
    stream = ["--op", "read:1m", "--connections", "1", "--depth", "32", "--duration", "14s", "--warmup", "1s"]
    for name in ["main", "inflight", "budgets", "drr"]:
        for _ in range(REPEATS):
            two_clients(name, ["--latency", "500us", "--bandwidth", "2G", "--buf-count", "256"],
                        stream, meta, label="E10 meta+stream")


def e2b_mechanism_cost():
    """Four connections, CPU-bound: the cost of every mechanism on its own."""
    for depth in [64, 512]:
        for name in ["inflight", "pending", "wtimeout", "databudget", "drr-only"]:
            bench("E2b getattr c4", name, [],
                  ["--op", "getattr", "--connections", "4", "--depth", str(depth),
                   "--duration", "6s", "--warmup", "1s"])


def e7b_quantum():
    """E7 with smaller DRR quanta: the light client waits for less of the heavy one."""
    heavy = ["--op", "getattr", "--connections", "1", "--depth", "2048", "--duration", "14s", "--warmup", "1s"]
    light = ["--op", "getattr", "--connections", "1", "--depth", "1", "--duration", "8s", "--warmup", "1s"]
    for quantum in ["8", "1"]:
        for _ in range(REPEATS):
            two_clients("drr", ["--scheduler", f"drr:{quantum}", "--latency", "1ms"], heavy, light,
                        label="E7b heavy+light quantum")


def e10b_deep_stream():
    """A streaming client with a deep queue (depth 256 of 1 MiB) next to a metadata
    client on 32 workers: FIFO serves 1 MiB reads ahead of metadata, DRR serves cost."""
    meta = ["--op", "getattr", "--connections", "1", "--depth", "64", "--duration", "8s", "--warmup", "1s"]
    stream = ["--op", "read:1m", "--connections", "1", "--depth", "256", "--duration", "14s", "--warmup", "1s"]
    server = ["--latency", "500us", "--bandwidth", "2G", "--buf-count", "512", "--concurrency", "32"]
    for name, extra in [("main", []), ("inflight", []), ("budgets", ["--max-data-per-conn", "512M"]),
                        ("drr", ["--max-data-per-conn", "512M"])]:
        for _ in range(REPEATS):
            two_clients(name, extra + server, stream, meta, label="E10b meta+deep stream")


if __name__ == "__main__":
    which = sys.argv[1:] or ["e1", "e2", "e4", "e5", "e6", "e7", "e8", "e9", "e10"]
    table = {"e1": e1_single_connection, "e2": e2_four_connections, "e4": e4_data_path,
             "e5": e5_non_reading_client, "e6": e6_allocator_hog, "e7": e7_heavy_light,
             "e8": e8_many_connections, "e9": e9_fast_slow, "e10": e10_cost_weighted,
             "e2b": e2b_mechanism_cost, "e7b": e7b_quantum, "e10b": e10b_deep_stream}
    for w in which:
        print(f"===== {w} =====", flush=True)
        table[w]()
