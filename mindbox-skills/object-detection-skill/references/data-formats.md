# Data Formats

## YOLO Format (Native)

This is what Ultralytics expects. If the dataset is already in this format, no conversion needed.

**Directory structure:**
```
dataset/
├── dataset.yaml
├── images/
│   ├── train/
│   │   ├── img001.jpg
│   │   └── ...
│   └── val/
│       ├── img100.jpg
│       └── ...
└── labels/
    ├── train/
    │   ├── img001.txt
    │   └── ...
    └── val/
        ├── img100.txt
        └── ...
```

**Label format** (one `.txt` per image, one line per object):
```
<class_id> <x_center> <y_center> <width> <height>
```
All values normalized to [0, 1] relative to image dimensions.

**dataset.yaml:**
```yaml
path: /mindbox/datasets/<dataset_name>
train: images/train
val: images/val
test: images/test  # optional

names:
  0: person
  1: car
  2: bicycle
```

## COCO JSON Format

**Structure:**
```
dataset/
├── annotations/
│   ├── instances_train.json
│   └── instances_val.json
└── images/
    ├── train/
    └── val/
```

**Conversion to YOLO:**
```python
from ultralytics.data.converter import convert_coco

convert_coco(
    labels_dir="/mindbox/datasets/<name>/annotations",
    save_dir="/mindbox/datasets/<name>/yolo_labels",
    use_segments=False
)
```

After conversion, create a `dataset.yaml` pointing to the converted labels.

## Pascal VOC XML Format

**Structure:**
```
dataset/
├── Annotations/
│   ├── img001.xml
│   └── ...
├── JPEGImages/
│   ├── img001.jpg
│   └── ...
└── ImageSets/Main/
    ├── train.txt
    └── val.txt
```

**Conversion script:**
```python
import xml.etree.ElementTree as ET
import os

def convert_voc_to_yolo(voc_dir, output_dir, class_names):
    """Convert VOC XML annotations to YOLO txt format."""
    os.makedirs(output_dir, exist_ok=True)
    for xml_file in os.listdir(os.path.join(voc_dir, "Annotations")):
        if not xml_file.endswith(".xml"):
            continue
        tree = ET.parse(os.path.join(voc_dir, "Annotations", xml_file))
        root = tree.getroot()
        img_w = int(root.find("size/width").text)
        img_h = int(root.find("size/height").text)

        txt_name = xml_file.replace(".xml", ".txt")
        with open(os.path.join(output_dir, txt_name), "w") as f:
            for obj in root.findall("object"):
                cls = class_names.index(obj.find("name").text)
                box = obj.find("bndbox")
                xmin, ymin = float(box.find("xmin").text), float(box.find("ymin").text)
                xmax, ymax = float(box.find("xmax").text), float(box.find("ymax").text)
                x_center = (xmin + xmax) / 2 / img_w
                y_center = (ymin + ymax) / 2 / img_h
                w = (xmax - xmin) / img_w
                h = (ymax - ymin) / img_h
                f.write(f"{cls} {x_center:.6f} {y_center:.6f} {w:.6f} {h:.6f}\n")
```

## Validation Checks

After preparing the data, verify:

1. **Every image has a label file** — missing labels mean missing annotations, not "no objects"
2. **Class IDs are contiguous from 0** — gaps cause silent training issues
3. **Coordinates are in [0, 1]** — un-normalized coordinates are a common bug
4. **Images are readable** — corrupt files crash training mid-epoch
5. **Train/val split exists** — training without validation means no early stopping or mAP tracking
