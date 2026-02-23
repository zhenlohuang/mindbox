# Hyperparameter Guide

## Defaults and Tuning Ranges

| Parameter | Default | Range | When to Change |
|-----------|---------|-------|----------------|
| `imgsz` | 640 | 320–1280 | Increase for small objects; decrease for edge deployment or OOM |
| `epochs` | 100 | 50–300 | Increase for small datasets; early stopping handles over-training |
| `batch` | 16 | 4–64 | Largest that fits in GPU memory; halve on OOM |
| `lr0` | 0.01 | 0.001–0.1 | Lower if training is unstable; higher for very large datasets |
| `lrf` | 0.01 | 0.001–0.1 | Final learning rate as fraction of lr0 |
| `patience` | 20 | 10–50 | Lower for fast iteration; higher if mAP improves slowly |
| `optimizer` | auto | SGD/AdamW/auto | YOLO26 defaults to MuSGD via "auto"; usually leave as-is |

## Data Augmentation

| Parameter | Default | Range | Effect |
|-----------|---------|-------|--------|
| `mosaic` | 1.0 | 0.0–1.0 | Combines 4 images; great for small objects and context diversity |
| `mixup` | 0.1 | 0.0–0.5 | Blends two images; regularization effect |
| `copy_paste` | 0.1 | 0.0–0.5 | Pastes objects across images; helps rare classes |
| `degrees` | 0.0 | 0.0–45.0 | Rotation; useful when objects appear at varied angles |
| `flipud` | 0.0 | 0.0–1.0 | Vertical flip; useful for aerial/satellite imagery |
| `fliplr` | 0.5 | 0.0–1.0 | Horizontal flip; safe default for most tasks |

## Scenario-Specific Recommendations

### Small dataset (< 500 images)
- Increase `epochs` to 200–300 (early stopping will prevent over-training)
- Enable aggressive augmentation: `mosaic=1.0`, `mixup=0.3`, `copy_paste=0.3`
- Use a smaller model (yolo26n/s) to reduce overfitting risk

### Small objects (components, defects, distant targets)
- Increase `imgsz` to 1280 — resolution matters most for small objects
- Keep `mosaic=1.0` — mosaic creates more small-object instances
- Consider multi-scale training if GPU memory allows

### Many classes (> 50)
- Increase `epochs` — more classes need more training signal
- Monitor per-class mAP in eval.json — some classes may need targeted augmentation

### Fast iteration / prototyping
- Use yolo26n, `epochs=30`, `imgsz=640`, `patience=10`
- Good enough to validate the pipeline before investing in a full training run
