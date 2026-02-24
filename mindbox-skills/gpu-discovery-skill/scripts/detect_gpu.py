#!/usr/bin/env python3
"""Detect GPU environment and write results to workspace/gpu_info.json."""

import json
import os
import subprocess
import sys


def query_gpus():
    """Query GPU information via nvidia-smi. Returns (gpus_list, cuda_version)."""
    try:
        result = subprocess.run(
            [
                "nvidia-smi",
                "--query-gpu=index,name,memory.total,memory.free,driver_version",
                "--format=csv,noheader,nounits",
            ],
            capture_output=True,
            text=True,
            timeout=10,
        )
        if result.returncode != 0:
            return [], None

        gpus = []
        for line in result.stdout.strip().splitlines():
            parts = [p.strip() for p in line.split(",")]
            if len(parts) < 5:
                continue
            gpus.append(
                {
                    "index": int(parts[0]),
                    "name": parts[1],
                    "memory_total_mb": int(parts[2]),
                    "memory_free_mb": int(parts[3]),
                    "driver_version": parts[4],
                }
            )

        # Query CUDA version separately
        cuda_version = None
        cuda_result = subprocess.run(
            ["nvidia-smi", "--query-gpu=driver_version", "--format=csv,noheader"],
            capture_output=True,
            text=True,
            timeout=10,
        )
        # Parse CUDA version from nvidia-smi header output
        header_result = subprocess.run(
            ["nvidia-smi"],
            capture_output=True,
            text=True,
            timeout=10,
        )
        if header_result.returncode == 0:
            for line in header_result.stdout.splitlines():
                if "CUDA Version" in line:
                    for part in line.split():
                        # Find the token right after "CUDA Version:"
                        if part.replace(".", "").replace("|", "").isdigit() and "." in part:
                            cuda_version = part.strip("|").strip()
                            break

        for gpu in gpus:
            gpu["cuda_version"] = cuda_version

        return gpus, cuda_version

    except (FileNotFoundError, subprocess.TimeoutExpired, Exception):
        return [], None


def main():
    gpus, _ = query_gpus()

    if gpus:
        report = {
            "mode": "gpu",
            "gpu_count": len(gpus),
            "gpus": gpus,
        }
    else:
        report = {
            "mode": "cpu",
            "gpu_count": 0,
            "gpus": [],
        }

    # Write JSON report
    os.makedirs("workspace", exist_ok=True)
    output_path = os.path.join("workspace", "gpu_info.json")
    with open(output_path, "w") as f:
        json.dump(report, f, indent=2)

    # Print human-readable summary
    print(f"Mode: {report['mode'].upper()}")
    if report["mode"] == "gpu":
        print(f"GPU count: {report['gpu_count']}")
        for gpu in report["gpus"]:
            print(
                f"  [{gpu['index']}] {gpu['name']} — "
                f"{gpu['memory_total_mb']} MB total, "
                f"{gpu['memory_free_mb']} MB free, "
                f"CUDA {gpu.get('cuda_version', 'N/A')}, "
                f"Driver {gpu['driver_version']}"
            )
    else:
        print("No NVIDIA GPU detected. Training will use CPU.")

    print(f"\nReport written to {output_path}")


if __name__ == "__main__":
    main()
