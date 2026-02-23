# YOLO Model Selection

YOLO comes in five sizes. All share the same NMS-free, end-to-end architecture — the difference
is depth and width, which trade off speed for accuracy.

## Benchmark Table (COCO val2017)

| Model | Params | mAP@0.5:0.95 | mAP@0.5 | CPU (ms) | GPU (ms) | Best For |
|-------|--------|---------------|---------|----------|----------|----------|
| yolo26n | ~2.5M | 40.9 | 57.1 | 38.9 | 1.4 | Edge/mobile, real-time, rapid prototyping |
| yolo26s | ~9M | 47.5 | 63.8 | 65.1 | 2.1 | Lightweight server, embedded devices |
| yolo26m | ~18M | 51.5 | 68.2 | 120.8 | 3.8 | General-purpose server deployment |
| yolo26l | ~40M | 53.1 | 70.0 | 165.3 | 5.2 | High-accuracy applications |
| yolo26x | ~65M | 54.2 | 71.3 | 260.7 | 7.8 | Maximum accuracy, resources not constrained |

## Selection Logic

Pick the largest model that fits comfortably in GPU memory during training. Fine-tuning uses
roughly 2–3x the memory of inference due to gradients and optimizer states.

**Rules of thumb:**

- **4 GB VRAM** → yolo26n only, batch=8, imgsz=640
- **8 GB VRAM** → yolo26s with batch=16, or yolo26m with batch=8
- **16 GB VRAM** → yolo26m with batch=16, or yolo26l with batch=8
- **24+ GB VRAM** → yolo26l/x with batch=16+

When in doubt, start with yolo26n — it trains fast, so you can validate the pipeline quickly
before scaling up to a larger model.

## When to Deviate from YOLO

YOLO is the right default for most detection tasks. Consider alternatives when:

- The user explicitly requests a different architecture (RT-DETR, Faster R-CNN, etc.)
- The task requires instance segmentation (use yolo26-seg variants)
- The task requires oriented bounding boxes (use yolo26-obb variants)
