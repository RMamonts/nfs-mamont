#!/usr/bin/env python3

"""
NFS Benchmark: nfs-mamont vs nfs-ganesha

Runs fio benchmarks across different block sizes and job counts,
with multiple iterations for statistical significance.

Test plan per (server, block_size, num_jobs) combination:

1. Start/restart NFS server
2. Sequential write
3. Sequential read
4. Repeated read (cached)
5. Restart server (flush read cache)
6. Random read/write (randrw)

Results are saved to CSV + JSON for analysis.
"""

import argparse
import csv
import json
import os
import subprocess
import sys
import time
import traceback
from collections import defaultdict
from dataclasses import asdict, dataclass
from datetime import datetime
from typing import Optional

# ── Configuration ───────────────────────────────────────────────────────────


@dataclass
class BenchConfig:
    block_sizes: Optional[list[str]] = None
    num_jobs_list: Optional[list[int]] = None
    iodepth_list: Optional[list[int]] = None
    direct_modes: Optional[list[int]] = None
    test_types: Optional[list[str]] = None
    size_per_job: str = "10G"
    iterations: int = 3
    ioengine: str = "libaio"
    mamont_project_dir: str = "/home/ubuntu/nfs-mamont"
    mamont_export_root: str = "/home/ubuntu"
    mamont_export_paths: str = "test"
    mamont_mount_export: str = "/test"
    mamont_mount_opts: str = (
        "vers=3,tcp,proto=tcp,port=2049,mountport=2049,nolock"
    )
    ganesha_export_path: str = "/home/ubuntu/test"
    ganesha_mount_export: str = "/test"
    ganesha_mount_opts: str = "vers=3,tcp,nolock"
    nfs_mount_point: str = "/mnt/nfs_test"
    nfs_server_ip: str = "10.78.119.148"
    nfs_data_ip: str = "10.0.1.2"
    server_user: str = "ubuntu"
    ssh_key: Optional[str] = None
    test_dir: str = "/mnt/nfs_test"
    output_dir: str = "bench/results"

    def __post_init__(self):
        if self.block_sizes is None:
            self.block_sizes = [
                "4k",
                "8k",
                "16k",
                "32k",
                "128k",
                "256k",
                "512k",
                "1M",
                "4M",
            ]

        if self.num_jobs_list is None:
            self.num_jobs_list = [1, 4, 16, 32]

        if self.iodepth_list is None:
            self.iodepth_list = [1, 8, 32, 64]

        if self.direct_modes is None:
            self.direct_modes = [0, 1]

        if self.test_types is None:
            self.test_types = ["read", "write", "randread", "randwrite", "randrw"]


CFG = BenchConfig()

# ── Remote commands helper ──────────────────────────────────────────────────


class RemoteExecutor:
    """Execute commands on the NFS server (local or remote via SSH)."""

    def __init__(
        self,
        host: Optional[str] = None,
        user: str = "ubuntu",
        ssh_key: Optional[str] = None,
    ):
        self.host = host
        self.user = user
        self.ssh_key = ssh_key

    def run(
        self,
        cmd: str,
        timeout: int = 120,
        check: bool = True,
    ) -> subprocess.CompletedProcess:
        if self.host and self.host not in ("127.0.0.1", "localhost"):
            ssh_cmd = ["ssh", "-o", "StrictHostKeyChecking=no"]
            if self.ssh_key:
                ssh_cmd += ["-i", self.ssh_key]
            ssh_cmd += [f"{self.user}@{self.host}", cmd]
            full_cmd = ssh_cmd
        else:
            full_cmd = ["bash", "-c", cmd]

        print(f" [exec] {cmd}")
        result = subprocess.run(
            full_cmd,
            capture_output=True,
            text=True,
            timeout=timeout,
        )

        if result.returncode != 0 and check:
            print(f" [exec] FAILED (rc={result.returncode})")
            if result.stdout.strip():
                print(f" [stdout] {result.stdout[-2000:]}")
            if result.stderr.strip():
                print(f" [stderr] {result.stderr[-2000:]}")
            raise subprocess.CalledProcessError(
                result.returncode,
                full_cmd,
                result.stdout,
                result.stderr,
            )

        return result


# ── Server management ───────────────────────────────────────────────────────


class NFSServer:
    """Base class for NFS server management."""

    def __init__(
        self,
        name: str,
        executor: RemoteExecutor,
        mount_export: str,
        mount_opts: str,
        mount_type: str = "nfs",
    ):
        self.name = name
        self.executor = executor
        self.mount_export = mount_export
        self.mount_opts = mount_opts
        self.mount_type = mount_type

    def start(self):
        raise NotImplementedError

    def stop(self):
        raise NotImplementedError

    def restart(self):
        self.stop()
        time.sleep(2)
        self.start()

    def is_running(self) -> bool:
        raise NotImplementedError

    def wait_ready(self, timeout: int = 30):
        """Wait until the NFS server is accepting connections."""
        deadline = time.time() + timeout
        while time.time() < deadline:
            if self.is_running():
                time.sleep(1)
                return
            time.sleep(0.5)

        raise TimeoutError(f"{self.name} did not become ready in {timeout}s")


class MamontServer(NFSServer):
    BENCH_CONFIG_PATH = "/tmp/nfs-mamont-bench.toml"

    def __init__(
        self,
        executor: RemoteExecutor,
        project_dir: str,
        export_root: str,
        export_paths: str,
        mount_export: str,
        mount_opts: str,
    ):
        super().__init__(
            "nfs-mamont",
            executor,
            mount_export=mount_export,
            mount_opts=mount_opts,
            mount_type="nfs",
        )
        self.project_dir = project_dir
        self.export_root = export_root
        self.export_paths = export_paths
        self._built = False

    def _source_env(self, cmd: str) -> str:
        return f"source $HOME/.cargo/env 2>/dev/null; {cmd}"

    def _write_config(self):
        """Generate mamont config on the remote server."""
        lines = [
            "[allocator]",
            "read_buffer_size = 1048576",
            "read_buffer_count = 512",
            "write_buffer_size = 1048576",
            "write_buffer_count = 256",
            "",
            "[exports]",
            f'root = "{self.export_root}"',
            f'paths = ["{self.export_paths}"]',
        ]
        content = "\\n".join(lines)
        self.executor.run(
            f"printf '{content}\\n' > {self.BENCH_CONFIG_PATH}",
            check=True,
        )
        print(f" Config written to remote: {self.BENCH_CONFIG_PATH}")

    def _ensure_built(self):
        if self._built:
            return

        print(f" Building {self.name} (cargo build --release)...")
        self.executor.run(
            self._source_env(
                f"cd {self.project_dir} && cargo build --release --example mirrorfs"
            ),
            timeout=600,
            check=True,
        )
        self._built = True
        print(" Build complete")

    def start(self):
        print(f" Starting {self.name} on remote server...")
        self.stop()
        time.sleep(1)

        self._ensure_built()
        self._write_config()

        binary = f"{self.project_dir}/target/release/examples/mirrorfs"
        cmd = (
            f"nohup {binary} -c {self.BENCH_CONFIG_PATH} "
            f"> /tmp/nfs-mamont.log 2>&1 & echo $!"
        )
        result = self.executor.run(cmd, check=True)
        remote_pid = result.stdout.strip().split("\n")[-1]
        print(f" {self.name} launched on remote (pid={remote_pid})")
        self.wait_ready(timeout=60)
        print(f" {self.name} is ready")

    def stop(self):
        print(f" Stopping {self.name} on remote server...")
        self.executor.run("pkill -f 'mirrorfs' || true", check=False)
        time.sleep(2)
        self.executor.run("pkill -9 -f 'mirrorfs' || true", check=False)
        time.sleep(1)

    def is_running(self) -> bool:
        result = self.executor.run(
            "ss -tln | grep -q ':2049'",
            check=False,
        )
        return result.returncode == 0


class GaneshaServer(NFSServer):
    GANESHA_CONF = "/etc/ganesha/ganesha.conf"

    def __init__(
        self,
        executor: RemoteExecutor,
        mount_export: str,
        mount_opts: str,
        export_path: str,
    ):
        super().__init__(
            "nfs-ganesha",
            executor,
            mount_export=mount_export,
            mount_opts=mount_opts,
            mount_type="nfs",
        )
        self.export_path = export_path

    def _write_config(self):
        """Generate ganesha config on the remote server."""
        lines = [
            "NFS_CORE_PARAM {",
            " mount_path_pseudo = true;",
            " Protocols = 3,4;",
            "}",
            "",
            "EXPORT_DEFAULTS {",
            " Access_Type = RW;",
            " Squash = No_Root_Squash;",
            " Sectype = sys;",
            "}",
            "",
            "EXPORT {",
            " Export_Id = 1;",
            f" Path = {self.export_path};",
            f" Pseudo = {self.mount_export};",
            " Protocols = 3,4;",
            " Access_Type = RW;",
            " Squash = No_Root_Squash;",
            " FSAL {",
            " Name = VFS;",
            " }",
            "}",
            "",
            "LOG {",
            " Default_Log_Level = WARN;",
            "}",
        ]
        content = "\\n".join(lines)
        self.executor.run(
            f"printf '{content}\\n' | sudo tee {self.GANESHA_CONF} > /dev/null",
            check=True,
        )
        print(
            f" Ganesha config written: export {self.export_path} -> "
            f"pseudo {self.mount_export}"
        )

    def start(self):
        print(f" Starting {self.name}...")
        self._write_config()
        self.executor.run("sudo systemctl restart nfs-ganesha", check=True)
        self.wait_ready()
        print(f" {self.name} started")

    def stop(self):
        print(f" Stopping {self.name}...")
        self.executor.run("sudo systemctl stop nfs-ganesha", check=False)
        time.sleep(2)

    def is_running(self) -> bool:
        result = self.executor.run(
            "systemctl is-active --quiet nfs-ganesha",
            check=False,
        )
        return result.returncode == 0


# ── FIO runner ──────────────────────────────────────────────────────────────


@dataclass
class FioResult:
    server: str
    test_type: str
    block_size: str
    num_jobs: int
    iteration: int
    direct: int
    iodepth: int
    bw_bytes: int = 0  # bandwidth in bytes/sec
    bw_mib: float = 0.0  # bandwidth in MiB/s
    iops: float = 0.0
    slat_mean_us: float = 0.0
    clat_mean_us: float = 0.0
    lat_mean_us: float = 0.0
    lat_p50_us: float = 0.0
    lat_p95_us: float = 0.0
    lat_p99_us: float = 0.0
    lat_p999_us: float = 0.0
    usr_cpu: float = 0.0
    sys_cpu: float = 0.0
    ctx: int = 0
    runtime_ms: int = 0
    read_bw_bytes: int = 0
    read_iops: float = 0.0
    write_bw_bytes: int = 0
    write_iops: float = 0.0


def run_fio(
    test_name: str,
    rw: str,
    bs: str,
    numjobs: int,
    iodepth: int,
    direct: int,
    directory: str,
    extra_args: Optional[list[str]] = None,
) -> dict:
    """Run a single fio test and return parsed JSON output."""
    cmd = [
        "fio",
        f"--name={test_name}",
        f"--rw={rw}",
        f"--bs={bs}",
        f"--numjobs={numjobs}",
        f"--iodepth={iodepth}",
        f"--size={CFG.size_per_job}",
        f"--direct={direct}",
        f"--ioengine={CFG.ioengine}",
        f"--directory={directory}",
        "--group_reporting",
        "--output-format=json",
        "--fallocate=none",
        "--randrepeat=0",
    ]
    if extra_args:
        cmd.extend(extra_args)

    print(f" [fio] {' '.join(cmd)}")
    result = subprocess.run(cmd, capture_output=True, text=True, timeout=3600)

    if result.returncode != 0:
        print(f" [fio] FAILED (rc={result.returncode})")
        print(f" stderr: {result.stderr[:500]}")
        return {}

    try:
        return json.loads(result.stdout)
    except json.JSONDecodeError:
        print(" [fio] Failed to parse JSON output")
        print(f" stdout (first 500 chars): {result.stdout[:500]}")
        return {}


def parse_fio_result(
    raw: dict,
    server: str,
    test_type: str,
    bs: str,
    numjobs: int,
    iteration: int,
    iodepth: int,
    direct: int,
) -> Optional[FioResult]:
    """Extract key metrics from fio JSON output."""
    if not raw or "jobs" not in raw:
        return None

    jobs = raw["jobs"]
    if not jobs:
        return None

    job = jobs[0]
    read_data = job.get("read", {})
    write_data = job.get("write", {})

    total_bw = read_data.get("bw_bytes", 0) + write_data.get("bw_bytes", 0)
    total_iops = read_data.get("iops", 0.0) + write_data.get("iops", 0.0)

    if read_data.get("bw_bytes", 0) > write_data.get("bw_bytes", 0):
        primary = read_data
    else:
        primary = write_data

    clat_ns = primary.get("clat_ns", {})
    slat_ns = primary.get("slat_ns", {})
    lat_ns = primary.get("lat_ns", {})
    lat_info = lat_ns if lat_ns.get("mean", 0) > 0 else clat_ns
    percentiles = lat_info.get("percentile", {})

    return FioResult(
        server=server,
        test_type=test_type,
        block_size=bs,
        num_jobs=numjobs,
        iteration=iteration,
        direct=direct,
        iodepth=iodepth,
        bw_bytes=total_bw,
        bw_mib=round(total_bw / (1024 * 1024), 2),
        iops=round(total_iops, 2),
        slat_mean_us=round(slat_ns.get("mean", 0) / 1000, 2),
        clat_mean_us=round(clat_ns.get("mean", 0) / 1000, 2),
        lat_mean_us=round(lat_info.get("mean", 0) / 1000, 2),
        lat_p50_us=round(percentiles.get("50.000000", 0) / 1000, 2),
        lat_p95_us=round(percentiles.get("95.000000", 0) / 1000, 2),
        lat_p99_us=round(
            percentiles.get("99.000000", 0) / 1000,
            2,
        ),
        lat_p999_us=round(percentiles.get("99.900000", 0) / 1000, 2),
        usr_cpu=round(job.get("usr_cpu", 0.0), 2),
        sys_cpu=round(job.get("sys_cpu", 0.0), 2),
        ctx=int(job.get("ctx", 0)),
        runtime_ms=primary.get("runtime", 0),
        read_bw_bytes=read_data.get("bw_bytes", 0),
        read_iops=round(read_data.get("iops", 0.0), 2),
        write_bw_bytes=write_data.get("bw_bytes", 0),
        write_iops=round(write_data.get("iops", 0.0), 2),
    )


# ── Mount / unmount helpers ─────────────────────────────────────────────────


def ensure_mount(mount_point: str, server: NFSServer):
    """Mount NFS export if not already mounted."""
    result = subprocess.run(
        ["mountpoint", "-q", mount_point],
        capture_output=True,
        check=False,
    )
    if result.returncode == 0:
        return

    subprocess.run(["sudo", "mkdir", "-p", mount_point], check=True, timeout=10)
    cmd = [
        "sudo",
        "mount",
        "-v",
        "-t",
        server.mount_type,
        "-o",
        server.mount_opts,
        f"{CFG.nfs_data_ip}:{server.mount_export}",
        mount_point,
    ]
    print(f" [mount] {' '.join(cmd)}")
    subprocess.run(cmd, check=True, timeout=30)
    time.sleep(1)


def unmount(mount_point: str):
    """Unmount NFS export."""
    result = subprocess.run(
        ["mountpoint", "-q", mount_point],
        capture_output=True,
        check=False,
    )
    if result.returncode != 0:
        return

    print(f" [umount] {mount_point}")
    subprocess.run(["sudo", "umount", "-f", mount_point], check=False, timeout=30)
    time.sleep(1)


def cleanup_test_files(directory: str):
    """Remove fio test files."""
    print(f" [cleanup] Removing test files in {directory}")
    subprocess.run(
        f"rm -f {directory}/testfile.*",
        shell=True,
        check=False,
        timeout=60,
    )
    subprocess.run(["sync"], check=False, timeout=30)
    time.sleep(1)


def drop_caches(server_executor: Optional[RemoteExecutor] = None):
    """Drop page cache on both client and server."""
    drop_cmd = "sync && sudo sh -c 'echo 3 > /proc/sys/vm/drop_caches'"

    print(" [cache] Dropping client caches")
    subprocess.run(drop_cmd, shell=True, check=False, timeout=10)

    if server_executor:
        print(" [cache] Dropping server caches")
        server_executor.run(drop_cmd, check=False)

    time.sleep(1)


# ── Main benchmark loop ─────────────────────────────────────────────────────


def run_test_case(
    server: NFSServer,
    test_dir: str,
    test_type: str,
    rw: str,
    bs: str,
    numjobs: int,
    iodepth: int,
    direct: int,
    iteration: int,
) -> Optional[FioResult]:
    """Run a single fio test case."""
    srv_name = server.name
    print(f"\n{'=' * 72}")
    print(
        f" [{srv_name}] {test_type} | bs={bs} jobs={numjobs} depth={iodepth} "
        f"direct={direct} iter={iteration}"
    )
    print(f"{'=' * 72}")
    extra_args = ["--rwmixread=50"] if rw == "randrw" else None
    raw = run_fio(
        test_name="testfile",
        rw=rw,
        bs=bs,
        numjobs=numjobs,
        iodepth=iodepth,
        direct=direct,
        directory=test_dir,
        extra_args=extra_args,
    )
    res = parse_fio_result(raw, srv_name, test_type, bs, numjobs, iteration, iodepth, direct)
    if res:
        print(
            f" => BW={res.bw_mib} MiB/s IOPS={res.iops} "
            f"slat={res.slat_mean_us}us clat={res.clat_mean_us}us p99={res.lat_p99_us}us"
        )
    else:
        print(" => FAILED or no data")
    return res


def build_servers(server_names: list[str]) -> list[NFSServer]:
    executor = RemoteExecutor(
        host=CFG.nfs_server_ip,
        user=CFG.server_user,
        ssh_key=CFG.ssh_key,
    )
    servers: list[NFSServer] = []

    if "mamont" in server_names:
        servers.append(
            MamontServer(
                executor=executor,
                project_dir=CFG.mamont_project_dir,
                export_root=CFG.mamont_export_root,
                export_paths=CFG.mamont_export_paths,
                mount_export=CFG.mamont_mount_export,
                mount_opts=CFG.mamont_mount_opts,
            )
        )

    if "ganesha" in server_names:
        servers.append(
            GaneshaServer(
                executor=executor,
                export_path=CFG.ganesha_export_path,
                mount_export=CFG.ganesha_mount_export,
                mount_opts=CFG.ganesha_mount_opts,
            )
        )

    return servers


def save_results(all_results: list[FioResult], output_dir: str):
    """Save results to CSV and JSON."""
    os.makedirs(output_dir, exist_ok=True)
    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")

    csv_path = os.path.join(output_dir, f"bench_{timestamp}.csv")
    json_path = os.path.join(output_dir, f"bench_{timestamp}.json")

    fields = [
        "server",
        "test_type",
        "block_size",
        "num_jobs",
        "iteration",
        "direct",
        "iodepth",
        "bw_bytes",
        "bw_mib",
        "iops",
        "slat_mean_us",
        "clat_mean_us",
        "lat_mean_us",
        "lat_p50_us",
        "lat_p95_us",
        "lat_p99_us",
        "lat_p999_us",
        "usr_cpu",
        "sys_cpu",
        "ctx",
        "runtime_ms",
        "read_bw_bytes",
        "read_iops",
        "write_bw_bytes",
        "write_iops",
    ]

    with open(csv_path, "w", newline="") as f:
        writer = csv.DictWriter(f, fieldnames=fields)
        writer.writeheader()
        for result in all_results:
            writer.writerow(asdict(result))

    with open(json_path, "w") as f:
        json.dump([asdict(result) for result in all_results], f, indent=2)

    print("\nResults saved to:")
    print(f" CSV: {csv_path}")
    print(f" JSON: {json_path}")
    return csv_path, json_path


def print_summary(results: list[FioResult]):
    """Print a summary table of average results per (server, test, bs, jobs, depth, direct)."""
    groups: dict[tuple, list[FioResult]] = defaultdict(list)
    for result in results:
        key = (
            result.server,
            result.test_type,
            result.block_size,
            result.num_jobs,
            result.iodepth,
            result.direct,
        )
        groups[key].append(result)

    print(f"\n{'=' * 128}")
    print(
        f"{'SERVER':<14} {'TEST':<10} {'BS':<8} {'JOBS':<6} {'DEPTH':<6} {'DIR':<4} "
        f"{'BW MiB/s':>10} {'IOPS':>10} {'P95 us':>10} {'P99 us':>10} {'N':>4}"
    )
    print(f"{'=' * 128}")

    for key in sorted(groups.keys()):
        items = groups[key]
        count = len(items)
        avg_bw = sum(result.bw_mib for result in items) / count
        avg_iops = sum(result.iops for result in items) / count
        avg_lat = sum(result.lat_p95_us for result in items) / count
        avg_p99 = sum(result.lat_p99_us for result in items) / count
        srv, test, bs, jobs, depth, direct = key
        print(
            f"{srv:<14} {test:<10} {bs:<8} {jobs:<6} {depth:<6} {direct:<4} "
            f"{avg_bw:>10.1f} {avg_iops:>10.1f} "
            f"{avg_lat:>10.1f} {avg_p99:>10.1f} {count:>4}"
        )

    print(f"{'=' * 128}")


def main():
    parser = argparse.ArgumentParser(
        description="NFS Benchmark: nfs-mamont vs nfs-ganesha",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Run this script on the CLIENT machine (10.78.126.238).
Server commands are sent via SSH to --server-host (default: 10.78.119.148).

Examples:
# Full benchmark (both servers, all combos, 10 iterations)
%(prog)s --servers mamont ganesha --iterations 10

# Quick test with mamont only
%(prog)s --servers mamont --block-sizes 1M --num-jobs 4 --iterations 2

# Custom server IP
%(prog)s --server-host 10.78.119.148 --servers mamont ganesha
""",
    )

    parser.add_argument(
        "--servers",
        nargs="+",
        choices=["mamont", "ganesha"],
        default=["mamont", "ganesha"],
        help="Which NFS servers to benchmark (default: both)",
    )
    parser.add_argument(
        "--fio-type",
        "--fio-types",
        dest="fio_types",
        nargs="+",
        choices=["read", "write", "randread", "randwrite", "randrw"],
        default=CFG.test_types,
        help=f"fio workload types to test (default: {CFG.test_types})",
    )
    parser.add_argument(
        "--block-sizes",
        nargs="+",
        default=CFG.block_sizes,
        help=f"Block sizes to test (default: {CFG.block_sizes})",
    )
    parser.add_argument(
        "--num-jobs",
        nargs="+",
        type=int,
        default=CFG.num_jobs_list,
        help=f"Number of fio jobs to test (default: {CFG.num_jobs_list})",
    )
    parser.add_argument(
        "--iodepths",
        nargs="+",
        type=int,
        default=CFG.iodepth_list,
        help=f"iodepth values to test (default: {CFG.iodepth_list})",
    )
    parser.add_argument(
        "--direct-modes",
        nargs="+",
        type=int,
        choices=[0, 1],
        default=CFG.direct_modes,
        help=f"direct modes to test (default: {CFG.direct_modes})",
    )
    parser.add_argument(
        "--iterations",
        type=int,
        default=CFG.iterations,
        help=f"Number of iterations per combo (default: {CFG.iterations})",
    )
    parser.add_argument(
        "--size",
        default=CFG.size_per_job,
        help=f"Size per fio job (default: {CFG.size_per_job})",
    )
    parser.add_argument(
        "--test-dir",
        default=CFG.nfs_mount_point,
        help=f"Directory for fio test files (default: {CFG.nfs_mount_point})",
    )
    parser.add_argument(
        "--output-dir",
        default=CFG.output_dir,
        help=f"Directory for result files (default: {CFG.output_dir})",
    )
    parser.add_argument(
        "--server-host",
        default=CFG.nfs_server_ip,
        help=f"NFS server host (default: {CFG.nfs_server_ip})",
    )
    parser.add_argument(
        "--server-user",
        default=CFG.server_user,
        help=f"SSH user for remote server commands (default: {CFG.server_user})",
    )
    parser.add_argument(
        "--ssh-key",
        default=CFG.ssh_key,
        help="SSH key for remote commands",
    )
    parser.add_argument(
        "--nfs-data-ip",
        default=CFG.nfs_data_ip,
        help=f"NFS data network IP for mount (default: {CFG.nfs_data_ip})",
    )
    parser.add_argument(
        "--mount-point",
        default=CFG.nfs_mount_point,
        help=f"Local NFS mount point (default: {CFG.nfs_mount_point})",
    )
    parser.add_argument(
        "--mamont-project-dir",
        default=CFG.mamont_project_dir,
        help=f"Project dir for nfs-mamont on server (default: {CFG.mamont_project_dir})",
    )
    parser.add_argument(
        "--mamont-export-root",
        default=CFG.mamont_export_root,
        help=f"Mamont config: export root dir (default: {CFG.mamont_export_root})",
    )
    parser.add_argument(
        "--mamont-export-paths",
        default=CFG.mamont_export_paths,
        help=f"Mamont config: export paths (default: {CFG.mamont_export_paths})",
    )
    parser.add_argument(
        "--mamont-mount-export",
        default=CFG.mamont_mount_export,
        help=f"NFS export path for mamont mount (default: {CFG.mamont_mount_export})",
    )
    parser.add_argument(
        "--mamont-mount-opts",
        default=CFG.mamont_mount_opts,
        help=f"Mount options for mamont (default: {CFG.mamont_mount_opts})",
    )
    parser.add_argument(
        "--ganesha-export-path",
        default=CFG.ganesha_export_path,
        help=f"Ganesha: local dir to export (default: {CFG.ganesha_export_path})",
    )
    parser.add_argument(
        "--ganesha-mount-export",
        default=CFG.ganesha_mount_export,
        help=f"NFS export path for ganesha mount (default: {CFG.ganesha_mount_export})",
    )
    parser.add_argument(
        "--ganesha-mount-opts",
        default=CFG.ganesha_mount_opts,
        help=f"Mount options for ganesha (default: {CFG.ganesha_mount_opts})",
    )
    parser.add_argument(
        "--local-mode",
        action="store_true",
        help="Run fio directly on server's local filesystem (skip NFS mount)",
    )

    args = parser.parse_args()

    CFG.size_per_job = args.size
    CFG.nfs_mount_point = args.mount_point
    CFG.nfs_server_ip = args.server_host
    CFG.nfs_data_ip = args.nfs_data_ip
    CFG.server_user = args.server_user
    CFG.ssh_key = args.ssh_key
    CFG.mamont_project_dir = args.mamont_project_dir
    CFG.mamont_export_root = args.mamont_export_root
    CFG.mamont_export_paths = args.mamont_export_paths
    CFG.mamont_mount_export = args.mamont_mount_export
    CFG.mamont_mount_opts = args.mamont_mount_opts
    CFG.ganesha_export_path = args.ganesha_export_path
    CFG.ganesha_mount_export = args.ganesha_mount_export
    CFG.ganesha_mount_opts = args.ganesha_mount_opts
    CFG.test_dir = args.test_dir
    CFG.output_dir = args.output_dir

    servers = build_servers(args.servers)
    if not servers:
        print("ERROR: No servers selected")
        sys.exit(1)

    total_combos = (
        len(servers)
        * len(args.fio_types)
        * len(args.block_sizes)
        * len(args.num_jobs)
        * len(args.iodepths)
        * len(args.direct_modes)
        * args.iterations
    )

    print(f"\n{'#' * 60}")
    print("# NFS Benchmark")
    print(
        f"# Server host: {CFG.nfs_server_ip} "
        f"(SSH: {CFG.server_user}@{CFG.nfs_server_ip})"
    )
    print("# Client: local (fio + mount)")
    print(f"# NFS servers: {[server.name for server in servers]}")
    print(f"# fio types: {args.fio_types}")
    print(f"# Block sizes: {args.block_sizes}")
    print(f"# Num jobs: {args.num_jobs}")
    print(f"# Iterations: {args.iterations}")
    print(f"# Size/job: {CFG.size_per_job}")
    print(f"# iodepths: {args.iodepths}")
    print(f"# direct modes: {args.direct_modes}")
    print(f"# Mount point: {CFG.nfs_mount_point} (via {CFG.nfs_data_ip})")
    print(f"# Total combos: {total_combos}")
    print(f"# Test dir: {CFG.test_dir}")
    print(f"{'#' * 60}\n")

    all_results: list[FioResult] = []
    combo_idx = 0

    try:
        for server in servers:
            for test_type in args.fio_types:
                for bs in args.block_sizes:
                    for num_jobs in args.num_jobs:
                        for iodepth in args.iodepths:
                            for direct in args.direct_modes:
                                for iteration in range(1, args.iterations + 1):
                                    combo_idx += 1
                                    print(f"\n{'#' * 60}")
                                    print(
                                        f"# Combo {combo_idx}/{total_combos}: {server.name} "
                                        f"{test_type} bs={bs} jobs={num_jobs} depth={iodepth} "
                                        f"direct={direct} iter={iteration}"
                                    )
                                    print(f"{'#' * 60}")

                                    test_dir = CFG.test_dir
                                    if args.local_mode:
                                        test_dir = f"{CFG.mamont_export_root}/{CFG.mamont_export_paths}"

                                    print(f"\n>>> Starting {server.name} for iteration {iteration}")
                                    server.restart()
                                    if not args.local_mode:
                                        ensure_mount(CFG.nfs_mount_point, server)
                                    subprocess.run(
                                        ["sudo", "mkdir", "-p", test_dir],
                                        check=False,
                                        timeout=10,
                                    )
                                    cleanup_test_files(test_dir)
                                    drop_caches(server.executor)

                                    result = run_test_case(
                                        server,
                                        test_dir,
                                        test_type,
                                        test_type,
                                        bs,
                                        num_jobs,
                                        iodepth,
                                        direct,
                                        iteration,
                                    )
                                    if result:
                                        all_results.append(result)

                                    cleanup_test_files(test_dir)

                                    if combo_idx % 5 == 0:
                                        save_results(all_results, CFG.output_dir)

                                    if not args.local_mode:
                                        unmount(CFG.nfs_mount_point)
                                    server.stop()

    except KeyboardInterrupt:
        print("\n\nInterrupted by user. Saving partial results...")
    except Exception as exc:
        print(f"\n\nError: {exc}")
        traceback.print_exc()
    finally:
        unmount(CFG.nfs_mount_point)
        for server in servers:
            try:
                server.stop()
            except Exception:
                pass

    if all_results:
        save_results(all_results, CFG.output_dir)
        print_summary(all_results)
    else:
        print("\nNo results collected.")


if __name__ == "__main__":
    main()
