#!/usr/bin/env python3
"""TCP segments per request (system-wide /proc/net/snmp; only the benchmark runs here)."""
import subprocess, sys, time
import run

def tcp():
    lines = [l.split() for l in open("/proc/net/snmp") if l.startswith("Tcp:")]
    hdr, val = lines[0], lines[1]
    d = dict(zip(hdr[1:], map(int, val[1:])))
    ext = [l.split() for l in open("/proc/net/netstat") if l.startswith("TcpExt:")]
    e = dict(zip(ext[0][1:], map(int, ext[1][1:])))
    return d["OutSegs"], e.get("DelayedACKs", 0), e.get("TCPPureAcks", 0)

CONFIGS = [("main", [], 128), ("branch", ["--max-in-flight", "32"], 128),
           ("main", [], 1024), ("branch", [], 1024), ("branch", ["--max-in-flight", "1048576"], 1024)]

for _ in range(int(sys.argv[1]) if len(sys.argv) > 1 else 1):
    for name, sargs, depth in CONFIGS:
        srv = run.Server(name, sargs)
        try:
            cmd = run.client_cmd(srv.port, ["--op", "getattr", "--connections", "1", "--depth", str(depth),
                                            "--duration", "6s", "--warmup", "1s"])
            cli = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
            time.sleep(2)
            t0, (s0, da0, pa0) = time.time(), tcp()
            time.sleep(3)
            t1, (s1, da1, pa1) = time.time(), tcp()
            out, _ = cli.communicate(timeout=60)
        finally:
            srv.stop()
        ops = next(iter(run.parse_table(out).values()))["ops"]
        n = ops * (t1 - t0)
        label = name + ("" if not sargs else " " + " ".join(sargs))
        print(f"{label:<30} depth {depth:>5}: {ops:>8.0f} ops/s | TCP segments per request (both directions) "
              f"{(s1 - s0) / n:5.3f}, requests per segment {n / max(s1 - s0, 1):5.1f}, delayed ACKs/req {(da1 - da0) / n:5.3f}", flush=True)
