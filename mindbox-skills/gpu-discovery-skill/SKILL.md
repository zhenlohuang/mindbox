---
name: gpu-discovery
description: >
  Detect GPU hardware and CUDA environment, output a structured report for downstream skills.
  Use this skill before any GPU-intensive task (training, inference, rendering) to determine
  available hardware and choose appropriate model sizes, batch sizes, and device settings.
compatibility:
  requires:
    - Bash
    - Python (>= 3.8)
  dependencies: []
---

# GPU Discovery

Detect the GPU environment and produce a machine-readable report that other skills can consume
to make hardware-aware decisions (model size, batch size, device selection).

## Workflow

```
1. Run detection script → 2. Read report → 3. Choose configuration
```

---

## Step 1: Run Detection Script

Run the detection script — it has zero external dependencies (stdlib only):

```bash
python scripts/detect_gpu.py
```

The script writes `gpu_info.json` and prints a human-readable summary to stdout.

---

## Step 2: Read the Report

Read `gpu_info.json`. The schema:

**GPU available:**
```json
{
  "mode": "gpu",
  "gpu_count": 1,
  "gpus": [{
    "index": 0,
    "name": "NVIDIA GeForce RTX 4090",
    "memory_total_mb": 24564,
    "memory_free_mb": 22000,
    "cuda_version": "12.8",
    "driver_version": "560.35.03"
  }]
}
```

**No GPU:**
```json
{
  "mode": "cpu",
  "gpu_count": 0,
  "gpus": []
}
```
