# Troubleshooting

## CUDA Out of Memory (OOM)

**Symptoms:** `RuntimeError: CUDA out of memory` during training, usually in the first few epochs.

**Why it happens:** The model, optimizer states, gradients, and a batch of images all need to fit
in GPU memory simultaneously. Larger batches and higher resolution images use more memory.

**Fixes (in order of preference):**
1. Halve `batch` (16 → 8 → 4) — most effective, minimal accuracy impact
2. Reduce `imgsz` (640 → 480 → 320) — trades accuracy for memory, especially hurts small objects
3. Use a smaller model (yolo26m → yolo26s → yolo26n)
4. Enable mixed precision (Ultralytics enables this by default, but verify `amp=True`)

## mAP Not Improving

**Symptoms:** mAP plateaus early or stays near zero.

**Possible causes and fixes:**
- **Bad annotations** — Spot-check a few label files against the images. Wrong class IDs or
  un-normalized coordinates are common. The data_prep.py validation should catch these.
- **Too few epochs** — Some datasets need 200+ epochs. Increase epochs and rely on patience
  for early stopping.
- **Learning rate too high** — If loss is noisy or diverging, reduce `lr0` by 10x.
- **Dataset too small** — With < 100 images, the model barely has signal. Maximize augmentation
  and use the smallest model.

## Small Object Detection

**Symptoms:** Large objects detected well, but small objects missed.

**Why:** At 640px input, objects smaller than ~32px occupy very few feature pixels. The model
literally can't see enough detail.

**Fixes:**
- Increase `imgsz` to 1280 — doubles the feature resolution for small objects
- Keep `mosaic=1.0` — mosaic augmentation creates more small-object training examples
- Consider tiling: slice large images into overlapping tiles during inference

## Overfitting

**Symptoms:** Training mAP keeps rising but validation mAP plateaus or drops.

**Fixes:**
- Enable early stopping (set `patience`, already in the default config)
- Increase data augmentation: raise `mosaic`, `mixup`, `copy_paste`
- Reduce model size — smaller models have less capacity to memorize
- Add more training data if possible

## Class Imbalance

**Symptoms:** Common classes detected well, rare classes missed.

**Fixes:**
- Use `copy_paste` augmentation — it effectively oversamples rare objects
- Manually duplicate images containing rare classes
- Check if the class is genuinely too rare (< 10 instances) — may need more data

## ONNX Export Failures

**Symptoms:** `model.export(format="onnx")` raises an error.

**Common causes:**
- Missing `onnx` package → `pip install onnx`
- Incompatible opset version → lower `opset` from 17 to 13 or 11
- Custom operations not supported → YOLO26 should be clean (no DFL), but check the error
  message for the specific unsupported op

## Training Hangs or Is Very Slow

**Symptoms:** Training starts but epochs take much longer than expected.

**Possible causes:**
- Data loading bottleneck → increase `workers` (default 8)
- Disk I/O too slow → move dataset to faster storage
- GPU underutilized → increase `batch` to saturate compute
- Wrong device → verify training is actually using GPU (`device=0`, not `device="cpu"`)
