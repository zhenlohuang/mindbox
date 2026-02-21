# Object Detection Fine-tuning

## 适用场景
- 自定义目标检测（工业缺陷、医学影像、自动驾驶等）
- 基于预训练检测模型微调到特定领域

## 推荐框架与模型
- **Ultralytics**: YOLO26, YOLOv11, YOLOv8 (推荐，开箱即用)
- **HuggingFace**: RT-DETR, DETR, Deformable DETR
- **Detectron2**: Faster R-CNN, RetinaNet

## 数据格式

### COCO JSON
- annotations 文件: {"images": [...], "annotations": [...], "categories": [...]}
- images 目录: 图片文件

### YOLO txt
- 每张图对应一个 .txt 文件，每行: class_id x_center y_center width height
- data.yaml: path, train, val, names

### 格式转换
- COCO → YOLO: 使用 ultralytics 内置转换或自行编写脚本
- VOC → YOLO: xml 标注转 txt

## 超参建议
- image_size: 640 (YOLO 默认)
- epochs: 100-300 (小数据集 100, 大数据集 300)
- batch_size: 16 (根据显存, A100 可用 32-64)
- optimizer: SGD (momentum=0.937) 或 AdamW
- lr: 0.01 (SGD) 或 0.001 (AdamW)
- augmentation: mosaic=1.0, mixup=0.15, copy_paste=0.3

## 评估指标
- 主指标: mAP@0.5:0.95
- 辅助指标: mAP@0.5, precision, recall
- 可视化: confusion matrix, PR curve, prediction samples

## 常见问题与修复
- OOM → 降低 batch_size 或 image_size
- mAP 不上升 → 检查标注质量, 增加 epochs
- 小目标漏检 → 增大 image_size, 使用多尺度训练
- 过拟合 → 增加数据增强, early stopping
- 类别不平衡 → 使用 class weights

## 模型导出
- ONNX: model.export(format='onnx')
- TensorRT: model.export(format='engine')
- CoreML: model.export(format='coreml')
- OpenVINO: model.export(format='openvino')
