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
python /home/mindbox/.claude/skills/gpu-discovery-skill/scripts/detect_gpu.py
```

The script writes `workspace/gpu_info.json` and prints a human-readable summary to stdout.

---

## Step 2: Read the Report

Read `workspace/gpu_info.json`. The schema:

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

---

## Step 3: Choose Configuration Based on Hardware

Use the `memory_free_mb` field from the report to select model sizes and batch sizes.

### VRAM Recommendation Table

| Free VRAM       | Recommended Model Size | Batch Size | Notes                        |
|-----------------|------------------------|------------|------------------------------|
| No GPU (CPU)    | nano (n)               | 4–8        | CPU-only, expect slow training |
| < 8 GB          | nano (n)               | 8–16       | Fits comfortably              |
| 8–16 GB         | small (s) / medium (m) | 16–32      | Good accuracy-speed balance   |
| 16–24 GB        | large (l)              | 32–64      | High accuracy                 |
| > 24 GB         | xlarge (x)             | 64+        | Maximum accuracy              |

### Multi-GPU

When `gpu_count > 1`, pass all GPU indices as a device list to enable DDP (Distributed Data
Parallel). Scale batch size linearly with GPU count.
