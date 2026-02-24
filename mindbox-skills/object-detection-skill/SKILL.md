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
---

# Object Detection Fine-tuning

This skill guides you through fine-tuning a YOLO model on a custom dataset and exporting the result
as an ONNX model. YOLO is an end-to-end architecture (no NMS post-processing) that exports cleanly
to ONNX — which is why it's the default choice here over older YOLO versions.

## Workflow Overview

```
0. Discover GPU → 1. Inspect dataset → 2. Prepare data → 3. Install deps → 4. Train → 5. Evaluate → 6. Export ONNX
```

Follow these steps in order. Each step produces outputs that feed the next.

---

## Step 0: Discover GPU

Before anything else, detect the available hardware so later steps can make informed choices
about model size and batch size.

1. Use the `gpu-discovery` skill to detect the GPU environment. It will produce
   `workspace/gpu_info.json` with hardware details.

2. Read `workspace/gpu_info.json` to determine the hardware mode (`gpu` or `cpu`).

3. Use the `memory_free_mb` value to guide model selection in Step 4:
   - **No GPU / < 8 GB** → `yolo26n`
   - **8–16 GB** → `yolo26s` or `yolo26m`
   - **> 16 GB** → `yolo26l` or `yolo26x`

---

## Step 1: Inspect the Dataset

Before writing any training code, understand what you're working with. Read the dataset directory
structure and identify the format — this determines whether conversion is needed.

Look for these patterns:
- **YOLO format** (ready to use): `images/` + `labels/` directories with a `dataset.yaml`
- **COCO format** (needs conversion): `annotations/*.json` + `images/`
- **VOC format** (needs conversion): `Annotations/*.xml` + `JPEGImages/`

Check `references/data-formats.md` for format details and conversion scripts.

Also look at basic dataset health: how many images, how many classes, whether train/val splits
exist. Unbalanced classes or missing splits will cause problems later — better to catch them now.

---

## Step 2: Prepare Data

Generate `workspace/scripts/data_prep.py` to handle format conversion (if needed) and validation.

The script should:
- Convert to YOLO format if the source is COCO or VOC
- Verify every image has a corresponding label file
- Create train/val splits if they don't already exist (80/20 default)
- Print dataset statistics: image count, class distribution, bounding box size distribution
- Produce or validate a `dataset.yaml` pointing to the correct paths

The `dataset.yaml` is what Ultralytics reads to find your data — getting the paths right here
saves debugging time during training.

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

## Step 3: Install Dependencies

Install the skill's dependencies from `scripts/requirements.txt`. Follow the dependency
installation procedure in the kernel agent documentation to create a virtual environment
and install:

```bash
uv venv workspace/.venv
source workspace/.venv/bin/activate
uv pip install -r /home/mindbox/.claude/skills/object-detection-skill/scripts/requirements.txt
```

Ultralytics pulls in PyTorch and all other transitive dependencies automatically. The `onnx`
package is needed later for export but installing it upfront avoids interrupting the pipeline.

---

## Step 4: Train

Generate `workspace/scripts/train.py` using the Ultralytics Python API.

### Pick the right model size

Read `workspace/gpu_info.json` (produced by Step 0) and use the `memory_free_mb` value to
select the appropriate model. Smaller models train faster and are easier to deploy; larger
models are more accurate. When the user doesn't specify, default to `yolo26n`.

| GPU Memory | Model | Why |
|-----------|-------|-----|
| < 8 GB | yolo26n | Fits comfortably, fast iteration |
| 8–16 GB | yolo26s/m | Good accuracy-speed balance |
| > 16 GB | yolo26l/x | Maximum accuracy |

See `references/model-selection.md` for detailed benchmarks.

### Training script structure

```python
from ultralytics import YOLO

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
    tensorboard=True,      # enables TensorBoard logging
    patience=20,           # early stopping — avoids wasting compute on plateaus
    save=True,
    save_period=10,
)
```

Adjust `batch` and `imgsz` based on GPU memory. If training OOMs, halve `batch` first, then
reduce `imgsz`. See `references/hyperparameters.md` for tuning guidance.

### TensorBoard logs

The Mindbox container exposes TensorBoard on port 6006 reading from `workspace/tb_logs/`.
Set the `TENSORBOARD_LOGDIR` environment variable before training so Ultralytics writes
events directly there:

```python
import os
os.makedirs("tb_logs", exist_ok=True)
os.environ["TENSORBOARD_LOGDIR"] = os.path.abspath("tb_logs")
```

Place this before `model.train(...)`. Since the script's cwd is `workspace/`, this creates
`workspace/tb_logs/`. With `tensorboard=True`, Ultralytics will write events straight there,
allowing the user to monitor training progress in real time.

---

## Step 5: Evaluate

Training already runs validation each epoch. For a final evaluation (especially on a held-out
test set), generate `workspace/scripts/eval.py`:

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

os.makedirs(os.path.join("..", "artifacts", "reports"), exist_ok=True)
with open(os.path.join("..", "artifacts", "reports", "eval.json"), "w") as f:
    json.dump(report, f, indent=2)
```

---

## Step 6: Export to ONNX

Generate `workspace/scripts/export.py`. YOLO's removal of DFL makes ONNX export particularly
clean — fewer custom ops means better compatibility across inference runtimes.

This step also collects all final artifacts into the `artifacts/` directory — weights, ONNX model,
and Ultralytics' auto-generated visualizations (confusion matrix, PR curve, training curves).

```python
from ultralytics import YOLO
import shutil, os, glob

# Export ONNX
model = YOLO("train_output/weights/best.pt")
model.export(format="onnx", imgsz=640, simplify=True, opset=17)

# Collect artifacts (artifacts/ lives under task_dir, one level up from workspace/)
artifacts = os.path.join("..", "artifacts")
os.makedirs(os.path.join(artifacts, "weights"), exist_ok=True)
os.makedirs(os.path.join(artifacts, "export"), exist_ok=True)
os.makedirs(os.path.join(artifacts, "reports"), exist_ok=True)

shutil.copy2("train_output/weights/best.pt", os.path.join(artifacts, "weights", "best.pt"))
shutil.copy2("train_output/weights/best.onnx", os.path.join(artifacts, "export", "model.onnx"))

for f in glob.glob("train_output/*.png"):  # confusion_matrix.png, PR_curve.png, results.png
    shutil.copy2(f, os.path.join(artifacts, "reports"))

# Copy TensorBoard logs to artifacts
shutil.copytree("tb_logs", os.path.join(artifacts, "tb_logs"), dirs_exist_ok=True)
```

Use `simplify=True` to reduce the ONNX graph — this improves inference performance on most
runtimes. Use `opset=17` for broad compatibility; lower it only if the target runtime requires it.

---

## Multi-GPU

If multiple GPUs are available, pass a device list:

```python
model.train(data="dataset.yaml", device=[0, 1, 2, 3], ...)
```

Ultralytics handles DDP (Distributed Data Parallel) automatically. Scale `batch` linearly with
GPU count (e.g., 4 GPUs → `batch=64`) to take full advantage of the parallelism.

---

## Final Artifact Checklist

Before marking the task complete, verify all outputs exist:

```
artifacts/
├── weights/best.pt           # Best checkpoint
├── reports/
│   ├── eval.json             # Evaluation metrics
│   ├── confusion_matrix.png  # Per-class confusion matrix
│   ├── PR_curve.png          # Precision-Recall curve
│   └── results.png           # Training loss/metric curves
├── export/model.onnx         # ONNX exported model
└── tb_logs/                   # TensorBoard logs (copied from workspace/tb_logs/)
    └── events.out.tfevents.*
```

---

## References

For detailed information on specific topics, read these files as needed:

- `references/model-selection.md` — Full model benchmark table with parameters, mAP, and inference speed
- `references/data-formats.md` — Data format specifications, conversion scripts, and dataset.yaml examples
- `references/hyperparameters.md` — Hyperparameter defaults, tuning ranges, and recommendations by scenario
- `references/troubleshooting.md` — Common issues (OOM, convergence, small objects, export failures) and fixes
