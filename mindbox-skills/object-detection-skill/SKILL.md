---
name: object-detection
description: >
  Fine-tune YOLO object detection models on custom datasets, export to ONNX, with TensorBoard logging.
  Use this skill whenever the user mentions object detection, YOLO fine-tuning, bounding box detection,
  物体检测, 目标检测, 缺陷检测, or wants to detect/locate/identify objects in images — even if they
  don't explicitly say "YOLO" or "object detection". Also applies to industrial defect detection,
  medical imaging detection, autonomous driving perception, and security surveillance scenarios.
compatibility:
  requires:
    - Bash
    - Python (>= 3.8)
  dependencies:
    - ultralytics
    - onnx
    - opencv-python-headless
---

# Object Detection Fine-tuning

This skill guides you through fine-tuning a YOLO model on a custom dataset and exporting the result
as an ONNX model. YOLO is an end-to-end architecture (no NMS post-processing) that exports cleanly
to ONNX — which is why it's the default choice here over older YOLO versions.

## Workflow Overview

```
0. Discover GPU → 1. Inspect dataset → 2. Prepare data → 3. Install deps → 4. Train → 5. Evaluate → 6. Export ONNX → 7. Collect artifacts → 8. Generate summary
```

Follow these steps in order. Each step produces outputs that feed the next.

---

## Step 0: Discover GPU

Use the `gpu-discovery` skill to detect the GPU environment. It will produce
`workspace/gpu_info.json` with hardware details. Use the `memory_free_mb` value to guide
model selection in Step 4.

---

## Step 1: Inspect the Dataset

Read the dataset directory structure and identify the format — this determines whether conversion is needed.

Look for these patterns:
- **YOLO format** (ready to use): `images/` + `labels/` directories with a `dataset.yaml`
- **COCO format** (needs conversion): `annotations/*.json` + `images/`
- **VOC format** (needs conversion): `Annotations/*.xml` + `JPEGImages/`

See `references/data-formats.md` for format details and conversion scripts.

Also check basic dataset health: image count, class count, whether train/val splits exist.

---

## Step 2: Prepare Data

Generate `workspace/scripts/data_prep.py` to handle format conversion (if needed) and validation.

The script should:
- Convert to YOLO format if the source is COCO or VOC
- Verify every image has a corresponding label file
- Create train/val splits if they don't already exist (80/20 default)
- Print dataset statistics: image count, class distribution, bounding box size distribution
- Produce or validate a `dataset.yaml` pointing to the correct paths

**Example `dataset.yaml`:**
```yaml
path: /mindbox/datasets/<dataset_name>
train: images/train
val: images/val
names:
  0: class_a
  1: class_b
```

---

## Step 3: Install deps

Install dependencies from `scripts/requirements.txt`
following the kernel agent's dependency rules.

Ultralytics pulls in PyTorch and all other transitive dependencies automatically.

---

## Step 4: Train

Generate `workspace/scripts/train.py` using the Ultralytics Python API.

### Pick the right model size

Read `workspace/gpu_info.json` (produced by Step 0) and select based on `memory_free_mb`.
When the user doesn't specify, default to `yolo26n`.

| GPU Memory | Model | Why |
|-----------|-------|-----|
| < 8 GB | yolo26n | Fits comfortably, fast iteration |
| 8–16 GB | yolo26s/m | Good accuracy-speed balance |
| > 16 GB | yolo26l/x | Maximum accuracy |

See `references/model-selection.md` for detailed benchmarks.

### Multi-GPU

If multiple GPUs are available, pass a device list and scale `batch` linearly with GPU count:

```python
model.train(data="dataset.yaml", device=[0, 1, 2, 3], batch=64, ...)
```

Ultralytics handles DDP (Distributed Data Parallel) automatically.

### Enable TensorBoard in Ultralytics

Before running training, enable TensorBoard in YOLO settings:

```bash
yolo settings tensorboard=True
```

### Training script structure

```python
import os
from ultralytics import YOLO

# TensorBoard: write events to workspace/tb_logs/
os.makedirs("tb_logs", exist_ok=True)
os.environ["TENSORBOARD_LOGDIR"] = os.path.abspath("tb_logs")

model = YOLO("yolo26n.pt")

results = model.train(
    data="/mindbox/datasets/<name>/dataset.yaml",
    epochs=100,
    imgsz=640,
    batch=16,
    device=0,
    project=".",
    name="train_output",
    exist_ok=True,
    patience=20,
    save=True,
    save_period=10,
)
```

Adjust `batch` and `imgsz` based on GPU memory. If training OOMs, halve `batch` first, then
reduce `imgsz`. See `references/hyperparameters.md` for tuning guidance.

---

## Step 5: Evaluate

For a final evaluation, generate `workspace/scripts/eval.py`:

```python
from ultralytics import YOLO
import json, os

model = YOLO("train_output/weights/best.pt")
metrics = model.val(data="dataset.yaml", split="test")

report = {
    "mAP50-95": float(metrics.box.map),
    "mAP50": float(metrics.box.map50),
    "precision": float(metrics.box.mp),
    "recall": float(metrics.box.mr),
}

os.makedirs("eval_output", exist_ok=True)
with open("eval_output/eval.json", "w") as f:
    json.dump(report, f, indent=2)
```

---

## Step 6: Export to ONNX

Generate `workspace/scripts/export.py`:

```python
from ultralytics import YOLO

model = YOLO("train_output/weights/best.pt")
model.export(format="onnx", imgsz=640, simplify=True, opset=17)
```

Use `simplify=True` to reduce the ONNX graph. Use `opset=17` for broad compatibility.

---

## Step 7: Collect Artifacts

Collect all final outputs into `artifacts/` (one level up from `workspace/`).
Run these commands directly from `workspace/`:

```bash
# Create directories
mkdir -p ../artifacts/weights ../artifacts/export ../artifacts/reports

# Weights
cp workspace/train_output/weights/best.pt ../artifacts/weights/best.pt

# ONNX model
cp workspace/train_output/weights/*.onnx ../artifacts/export/model.onnx

# Evaluation report
cp workspace/eval_output/eval.json ../artifacts/reports/eval.json

# Ultralytics visualizations (confusion matrix, PR curve, training curves)
cp workspace/train_output/*.png ../artifacts/reports/

# TensorBoard logs
cp -r workspace/tb_logs ../artifacts/tb_logs
```

After running, verify all outputs exist:

```
artifacts/
├── weights/best.pt           # Best checkpoint
├── reports/
│   ├── eval.json             # Evaluation metrics
│   ├── confusion_matrix.png  # Per-class confusion matrix
│   ├── PR_curve.png          # Precision-Recall curve
│   └── results.png           # Training loss/metric curves
├── export/model.onnx         # ONNX exported model
└── tb_logs/                  # TensorBoard logs
    └── events.out.tfevents.*
```

---

## Step 8: Generate Summary

Write `summary.md` in the task working directory (`{task_dir}/summary.md`) covering:
- **Task**: dataset name, model variant, task type (object detection)
- **Hardware**: GPU(s) used or CPU-only
- **Key hyperparameters**: epochs, batch size, imgsz, patience, model variant
- **Results**: mAP50-95, mAP50, precision, recall (from `artifacts/reports/eval.json`)
- **Artifacts produced**: list of files in `artifacts/` with their sizes
- **Issues encountered**: any errors that required retries and how they were resolved, or "None"

---

## References

For detailed information on specific topics, read these files as needed:

- `references/model-selection.md` — Model benchmarks (parameters, mAP, inference speed)
- `references/data-formats.md` — Data format specs, conversion scripts, dataset.yaml examples
- `references/hyperparameters.md` — Hyperparameter defaults, tuning ranges, recommendations
- `references/troubleshooting.md` — Common issues (OOM, convergence, small objects, export failures) and fixes
