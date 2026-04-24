#!/usr/bin/env python3

"""
Analyze benchmark results: aggregate, filter outliers, produce comparison tables
and plots.

Usage:
python3 bench/analyze.py bench/results/bench_*.json
python3 bench/analyze.py bench/results/bench_*.json --plot
"""

import argparse
import json
import math
import os
from collections import defaultdict


def load_results(paths: list[str]) -> list[dict]:
    all_results = []
    for path in paths:
        with open(path) as file:
            data = json.load(file)
        all_results.extend(data)
    return all_results


def remove_outliers_iqr(values: list[float], factor: float = 1.5) -> list[float]:
    """Remove outliers using the IQR method."""
    if len(values) < 4:
        return values

    sorted_values = sorted(values)
    q1 = sorted_values[len(sorted_values) // 4]
    q3 = sorted_values[3 * len(sorted_values) // 4]
    iqr = q3 - q1
    lower = q1 - factor * iqr
    upper = q3 + factor * iqr
    return [value for value in values if lower <= value <= upper]


def aggregate(results: list[dict], metric: str = "bw_mib") -> dict:
    """Group by (server, test_type, block_size, num_jobs, iodepth, direct) and compute stats."""
    groups: dict[tuple, list[float]] = defaultdict(list)
    for result in results:
        key = (
            result["server"],
            result["test_type"],
            result["block_size"],
            result["num_jobs"],
            result.get("iodepth", 0),
            result.get("direct", 0),
        )
        groups[key].append(result[metric])

    stats = {}
    for key, values in groups.items():
        clean = remove_outliers_iqr(values)
        count = len(clean)
        if count == 0:
            continue

        mean = sum(clean) / count
        variance = sum((value - mean) ** 2 for value in clean) / count if count > 1 else 0
        stddev = math.sqrt(variance)
        stats[key] = {
            "mean": round(mean, 2),
            "stddev": round(stddev, 2),
            "min": round(min(clean), 2),
            "max": round(max(clean), 2),
            "n": count,
            "n_outliers": len(values) - count,
        }

    return stats


def print_comparison_table(
    results: list[dict],
    metric: str = "bw_mib",
    metric_label: str = "BW (MiB/s)",
):
    """Print a side-by-side comparison of servers."""
    stats = aggregate(results, metric)

    servers = sorted({key[0] for key in stats})
    tests = sorted({key[1] for key in stats})
    block_sizes = sorted({key[2] for key in stats}, key=parse_size)
    jobs_list = sorted({key[3] for key in stats})
    depths = sorted({key[4] for key in stats})
    directs = sorted({key[5] for key in stats})

    print(f"\n{'=' * 120}")
    print(f" Metric: {metric_label}")
    print(f"{'=' * 120}")

    for test in tests:
        for depth in depths:
            for direct in directs:
                print(f"\n Test: {test} | depth={depth} direct={direct}")
                print(f" {'BS':<8} {'JOBS':<6}", end="")
                for server in servers:
                    print(f" | {server:>20}", end="")
                if len(servers) == 2:
                    print(f" | {'diff %':>10}", end="")
                print()

                print(f" {'-' * 8} {'-' * 6}", end="")
                for _ in servers:
                    print(f" | {'-' * 20}", end="")
                if len(servers) == 2:
                    print(f" | {'-' * 10}", end="")
                print()

                for block_size in block_sizes:
                    for num_jobs in jobs_list:
                        print(f" {block_size:<8} {num_jobs:<6}", end="")
                        values = []

                        for server in servers:
                            key = (server, test, block_size, num_jobs, depth, direct)
                            stat = stats.get(key)
                            if stat:
                                print(
                                    f" | {stat['mean']:>9.1f} ±{stat['stddev']:>6.1f} "
                                    f"(n={stat['n']})",
                                    end="",
                                )
                                values.append(stat["mean"])
                            else:
                                print(f" | {'N/A':>20}", end="")
                                values.append(None)

                        if len(servers) == 2 and all(value is not None and value > 0 for value in values):
                            diff = ((values[0] - values[1]) / values[1]) * 100
                            sign = "+" if diff >= 0 else ""
                            print(f" | {sign}{diff:>8.1f}%", end="")
                        elif len(servers) == 2:
                            print(f" | {'N/A':>10}", end="")

                        print()


def parse_size(size: str) -> int:
    """Parse size string to bytes for sorting."""
    normalized = size.lower().strip()
    multipliers = {"k": 1024, "m": 1024**2, "g": 1024**3}

    for suffix, multiplier in multipliers.items():
        if normalized.endswith(suffix):
            return int(float(normalized[:-1]) * multiplier)

    return int(normalized)


def plot_results(results: list[dict], output_dir: str):
    """Generate comparison plots using matplotlib."""
    try:
        import matplotlib

        matplotlib.use("Agg")
        import matplotlib.pyplot as plt
        import numpy as np
    except ImportError:
        print("matplotlib/numpy not installed. Skipping plots.")
        print(" pip install matplotlib numpy")
        return

    os.makedirs(output_dir, exist_ok=True)
    tests = sorted({result["test_type"] for result in results})
    servers = sorted({result["server"] for result in results})
    block_sizes = sorted({result["block_size"] for result in results}, key=parse_size)
    jobs_list = sorted({result["num_jobs"] for result in results})
    depths = sorted({result.get("iodepth", 0) for result in results})
    directs = sorted({result.get("direct", 0) for result in results})

    colors = {
        "nfs-mamont": "#2196F3",
        "nfs-ganesha": "#FF9800",
    }

    for metric, label, unit in [
        ("bw_mib", "Bandwidth", "MiB/s"),
        ("iops", "IOPS", "ops"),
        ("lat_mean_us", "Mean Latency", "us"),
    ]:
        stats = aggregate(results, metric)

        for test in tests:
            for depth in depths:
                for direct in directs:
                    fig, axes = plt.subplots(
                        1,
                        len(jobs_list),
                        figsize=(5 * len(jobs_list), 5),
                        sharey=True,
                        squeeze=False,
                    )
                    fig.suptitle(
                        f"{label} — {test} — depth={depth} direct={direct}",
                        fontsize=14,
                        fontweight="bold",
                    )

                    for job_index, num_jobs in enumerate(jobs_list):
                        ax = axes[0][job_index]
                        x = np.arange(len(block_sizes))
                        width = 0.35

                        for server_index, server in enumerate(servers):
                            means = []
                            errors = []
                            for block_size in block_sizes:
                                key = (server, test, block_size, num_jobs, depth, direct)
                                stat = stats.get(key)
                                means.append(stat["mean"] if stat else 0)
                                errors.append(stat["stddev"] if stat else 0)

                            offset = (server_index - (len(servers) - 1) / 2) * width
                            ax.bar(
                                x + offset,
                                means,
                                width,
                                yerr=errors,
                                label=server,
                                color=colors.get(server, f"C{server_index}"),
                                alpha=0.85,
                                capsize=3,
                            )

                        ax.set_title(f"jobs={num_jobs}")
                        ax.set_xlabel("Block Size")
                        ax.set_xticks(x)
                        ax.set_xticklabels(block_sizes, rotation=45)
                        if job_index == 0:
                            ax.set_ylabel(f"{label} ({unit})")
                        ax.legend(fontsize=8)
                        ax.grid(axis="y", alpha=0.3)

                    plt.tight_layout()
                    file_name = f"{metric}_{test}_d{depth}_dir{direct}.png"
                    file_path = os.path.join(output_dir, file_name)
                    plt.savefig(file_path, dpi=150)
                    plt.close()
                    print(f" Plot saved: {file_path}")

    # Heatmap: mamont speedup over ganesha
    if len(servers) == 2:
        bandwidth_stats = aggregate(results, "bw_mib")
        default_depth = depths[0] if depths else 0
        default_direct = directs[0] if directs else 0
        for test in tests:
            matrix = []
            y_labels = []
            for num_jobs in jobs_list:
                row = []
                for block_size in block_sizes:
                    key_left = (servers[0], test, block_size, num_jobs, default_depth, default_direct)
                    key_right = (servers[1], test, block_size, num_jobs, default_depth, default_direct)
                    stat_left = bandwidth_stats.get(key_left)
                    stat_right = bandwidth_stats.get(key_right)
                    if stat_left and stat_right and stat_right["mean"] > 0:
                        row.append(
                            ((stat_left["mean"] - stat_right["mean"]) / stat_right["mean"])
                            * 100
                        )
                    else:
                        row.append(0)
                matrix.append(row)
                y_labels.append(f"jobs={num_jobs}")

            if not matrix:
                continue

            fig, ax = plt.subplots(figsize=(8, 4))
            data = np.array(matrix)
            vmax = max(abs(data.min()), abs(data.max()), 1)
            image = ax.imshow(
                data,
                cmap="RdYlGn",
                vmin=-vmax,
                vmax=vmax,
                aspect="auto",
            )

            ax.set_xticks(range(len(block_sizes)))
            ax.set_xticklabels(block_sizes)
            ax.set_yticks(range(len(y_labels)))
            ax.set_yticklabels(y_labels)
            ax.set_xlabel("Block Size")
            ax.set_title(
                f"BW difference: {servers[0]} vs {servers[1]} — {test}\n"
                f"(green = {servers[0]} faster, red = {servers[1]} faster)"
            )

            for row_index in range(len(y_labels)):
                for col_index in range(len(block_sizes)):
                    value = data[row_index, col_index]
                    ax.text(
                        col_index,
                        row_index,
                        f"{value:+.1f}%",
                        ha="center",
                        va="center",
                        fontsize=9,
                        color="black" if abs(value) < vmax * 0.5 else "white",
                    )

            plt.colorbar(image, label="% difference")
            plt.tight_layout()
            file_path = os.path.join(
                output_dir,
                f"heatmap_bw_{test}_d{default_depth}_dir{default_direct}.png",
            )
            plt.savefig(file_path, dpi=150)
            plt.close()
            print(f" Heatmap saved: {file_path}")


def main():
    parser = argparse.ArgumentParser(description="Analyze NFS benchmark results")
    parser.add_argument("files", nargs="+", help="JSON result files")
    parser.add_argument(
        "--plot",
        action="store_true",
        help="Generate plots (needs matplotlib)",
    )
    parser.add_argument(
        "--plot-dir",
        default="bench/results/plots",
        help="Directory for plots",
    )
    parser.add_argument(
        "--metric",
        default="bw_mib",
        choices=[
            "bw_mib",
            "iops",
            "slat_mean_us",
            "clat_mean_us",
            "lat_mean_us",
            "lat_p95_us",
            "lat_p99_us",
            "lat_p999_us",
            "usr_cpu",
            "sys_cpu",
            "ctx",
        ],
        help="Primary metric for tables",
    )
    args = parser.parse_args()

    results = load_results(args.files)
    print(f"Loaded {len(results)} data points from {len(args.files)} file(s)")

    metric_labels = {
        "bw_mib": "Bandwidth (MiB/s)",
        "iops": "IOPS",
        "slat_mean_us": "Submission Latency (us)",
        "clat_mean_us": "Completion Latency (us)",
        "lat_mean_us": "Mean Latency (us)",
        "lat_p95_us": "P95 Latency (us)",
        "lat_p99_us": "P99 Latency (us)",
        "lat_p999_us": "P99.9 Latency (us)",
        "usr_cpu": "User CPU (%)",
        "sys_cpu": "System CPU (%)",
        "ctx": "Context Switches",
    }

    for metric, label in metric_labels.items():
        print_comparison_table(results, metric, label)

    if args.plot:
        print("\nGenerating plots...")
        plot_results(results, args.plot_dir)


if __name__ == "__main__":
    main()
