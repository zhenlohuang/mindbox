# Mindbox 系统概要设计

> 版本：0.1-draft | 日期：2025-02-14

---

## 1. 项目定位

Mindbox 是一个以 Docker 容器形式交付的**自动化模型微调平台**。用户通过 CLI 描述微调任务，系统借助 AI Kernel（Claude Code / Codex）自动完成数据准备、训练脚本生成、超参搜索、训练执行、评估与模型导出的全流程。

### 1.1 设计目标

| 目标 | 说明 |
|------|------|
| **低门槛** | 用户只需提供数据集与任务描述，无需手写训练代码 |
| **可复现** | 每次 task 记录完整快照（超参、数据版本、代码版本、基线权重），可精确重放 |
| **可观测** | 实时日志、TensorBoard、评估报告一体化 |
| **可扩展** | Skills 体系支持接入新的微调范式（LoRA、QLoRA、Full FT、RLHF 等） |

---

## 2. 核心概念

```
User ──▶ Task ──▶ Artifact
         │          │
         │          ├── 超参配置
         │          ├── 数据版本
         │          ├── 训练日志
         │          └── 评估报告
         │
         └── 共享：数据集、基线模型
```

| 概念 | 说明 | 标识 |
|------|------|------|
| **Task** | 一次完整的微调执行，包含从数据准备到模型导出的全过程 | `task_id`（自动生成，如 `task-20250214-001`） |
| **Artifact** | Task 产出物的集合：权重、评估报告、导出模型 | 存储在 `tasks/{task_id}/artifacts/` 下 |

---

## 3. 系统架构

```
┌─────────────────────────────────────────────────────────┐
│              用户终端（本地或远程）                         │
│                                                         │
│  ┌───────────┐                                          │
│  │ mindbox-cli│  ◀── 用户交互入口                        │
│  └─────┬─────┘                                          │
│        │                                                │
└────────┼────────────────────────────────────────────────┘
         │ HTTP / SSE
         ▼
┌─────────────────────────────────────────────────────────┐
│                 Mindbox Docker Container                 │
│                                                         │
│  ┌───────────┐                                         │
│  │  mindbox   │  ◀── REST API + SSE 服务                  │
│  │  server    │      全局任务锁（单任务串行）              │
│  └─────┬─────┘                                         │
│        │                                                │
│        ▼                                                │
│  ┌───────────┐     ┌────────────────┐                  │
│  │  mindbox   │────▶│  mindbox-skills │                  │
│  │  kernel    │     │  (SKILL.md集合) │                  │
│  │            │     └────────────────┘                  │
│  │ ┌────────┐│                                         │
│  │ │claude- ││  ◀── MINDBOX_KERNEL 环境变量选择          │
│  │ │code    ││                                         │
│  │ ├────────┤│                                         │
│  │ │codex   ││                                         │
│  │ └────────┘│                                         │
│  └─────┬─────┘                                         │
│        │                                                │
│        ▼                                                │
│  ┌──────────────────────────────────┐                  │
│  │         执行环境                   │                  │
│  │  Python / PyTorch / HuggingFace  │                  │
│  │  GPU Runtime (CUDA) - 单机多卡    │                  │
│  └──────────────────────────────────┘                  │
└─────────────────────────────────────────────────────────┘
```

### 3.1 组件职责

#### mindbox-cli

用户侧命令行工具，可运行在 Host 或远程机器上，通过 HTTP/SSE 与 Server 通信。

- 容器（Sandbox）的启停管理
- 任务的 CRUD 管理
- 提交微调任务（自然语言 + 配置文件）
- 实时流式查看训练日志（SSE）
- 查询与下载 artifact

命令列表：

```bash
# ── Sandbox ──

# 启动 sandbox，name 可选（默认自动生成）
mindbox sandbox start [--name <sandbox_name>]

# 停止 sandbox
mindbox sandbox stop [--name <sandbox_name>]

# 销毁 sandbox（停止并删除容器，挂载的数据不受影响）
mindbox sandbox destroy [--name <sandbox_name>]

# ── Task ──

# 创建并启动任务
#   --dataset: 数据集路径
#   --desc:    任务描述（自然语言）
mindbox task create \
  --dataset /datasets/reviews.jsonl \
  --desc "基于 Qwen2.5-7B 做情感分类微调，使用 LoRA"

# 停止正在运行的任务
mindbox task stop <task_id>

# 连接到正在运行的任务（实时查看日志与进度）
mindbox task attach <task_id>

# 列出任务
mindbox task list
```

#### mindbox-server

运行在容器内的 API 服务，是系统的控制平面。

核心职责：

- 接收并校验 CLI 请求（支持远程 HTTP/SSE 连接）
- 管理 task 生命周期与状态机
- **全局任务锁**：同一时刻仅允许一个 task 执行，新请求排队或返回 busy
- 根据环境变量 `MINDBOX_KERNEL` 初始化对应的 Kernel 实现
- 调度 kernel 执行微调任务
- 提供日志流（SSE）、状态查询、artifact 下载接口
- 持久化任务元数据（task.yaml）
- 对已完成/失败的 task 目录做写保护，防止误删历史数据

#### mindbox-kernel

系统的"大脑"，通过 `MINDBOX_KERNEL` 环境变量选择底层推理引擎。

支持的引擎：

| 引擎 | 环境变量值 | 说明 |
|------|-----------|------|
| Claude Code | `claude-code` | Anthropic 的代码智能体 |
| Codex | `codex` | OpenAI 的代码智能体 |

核心职责：

- 解析用户任务描述
- 根据匹配的 skill 生成训练方案（数据处理脚本、训练脚本、评估脚本）
- 执行并监控训练过程
- 异常诊断与自动修复（如 OOM 时自动降低 batch size）
- 单机多卡适配（自动检测 GPU 数量，选择 DataParallel / FSDP 策略）
- 生成评估报告

工作模式：

```
用户 task + 数据集元信息 + 匹配的 SKILL.md
        │
        ▼
   Kernel 推理（Claude Code / Codex）
        │
        ├── 生成 data_prep.py     → 数据预处理
        ├── 生成 train.py         → 训练主脚本
        ├── 生成 eval.py          → 评估脚本
        └── 生成 config.json       → 运行配置
        │
        ▼
   Kernel 执行生成的脚本 → 监控 → 收集结果
```

#### mindbox-skills

预定义的微调知识库，以 SKILL.md 文件组织。Skills 按**任务类型**和**微调方法**两个维度组织，Agent 根据用户任务描述匹配对应的 SKILL.md 注入上下文。

```
mindbox-skills/
  # ── 任务类型（每个注入 Agent 上下文时独立使用）──
  object-detection/SKILL.md       # YOLO26, RT-DETR, Faster R-CNN
  image-classification/SKILL.md   # ViT, ResNet, EfficientNet
  instance-segmentation/SKILL.md  # Mask R-CNN, YOLO-seg
  text-classification/SKILL.md    # BERT, Qwen 分类微调
  seq2seq/SKILL.md                # 翻译、摘要、问答
  rlhf/SKILL.md                   # 对齐训练

  # ── 微调方法（可与任务类型组合使用）──
  lora/SKILL.md                   # LoRA/QLoRA 微调方法
  full-ft/SKILL.md                # 全量微调方法
```

**匹配逻辑**：

1. Agent 从任务描述中识别任务类型 → 匹配任务类型 SKILL
2. 如果用户指定了微调方法（如 "使用 LoRA"），额外注入方法 SKILL
3. 如果用户未指定方法，Agent 根据任务类型 SKILL 中的建议自行选择

SKILL.md 内容结构（NLP 示例 — LoRA）：

```markdown
# LoRA Fine-tuning Skill

## 适用场景
- 基座模型参数量 > 1B
- GPU 显存有限（< 24GB）
- 需要快速迭代

## 超参建议
- rank: 8-64
- alpha: 16-128
- target_modules: 根据模型架构自动选择

## 数据格式要求
- JSONL，每行包含 instruction / input / output

## 常见问题与修复策略
- OOM → 降低 batch size 或 rank
- loss 不收敛 → 检查学习率、数据质量
```

SKILL.md 内容结构（CV 示例 — Object Detection）：

```markdown
# Object Detection Fine-tuning

## 适用场景
- 自定义目标检测（工业缺陷、医学影像、自动驾驶等）

## 推荐框架与模型
- Ultralytics: YOLO26, YOLOv11, YOLOv8 (推荐，开箱即用)
- HuggingFace: RT-DETR, DETR, Deformable DETR
- Detectron2: Faster R-CNN, RetinaNet

## 数据格式
- COCO JSON / YOLO txt / VOC XML
- 格式转换方法

## 评估指标
- 主指标: mAP@0.5:0.95
- 辅助指标: mAP@0.5, precision, recall
- 可视化: confusion matrix, PR curve, prediction samples
```

---

## 4. 数据目录结构

```
/mindbox/
├── datasets/                              # 数据集目录（只读挂载，用户自行管理）
│
├── models/                                # 基线模型缓存
│   └── {model_name}/
│
├── skills/                                # 微调技能库
│   └── {skill_name}/SKILL.md
│
└── tasks/
    └── {task_id}/
        ├── AGENTS.md             # Kernel 生成的 Agent 指令
        ├── CLAUDE.md -> AGENTS.md # 软链接，Claude Code 自动读取
        ├── task.yaml              # 本次任务完整快照
        ├── workspace/             # Agent 可写工作区（脚本执行的 cwd）
        │   └── scripts/           # kernel 生成的脚本与配置
        ├── logs/
        │   ├── kernel.log         # Kernel (Claude Code) 输出日志
        │   ├── train.log          # 训练脚本输出（Agent 重定向）
        │   └── ...                # 其他脚本输出日志
        ├── tb_logs/               # TensorBoard 日志
        └── artifacts/             # 最终产物
            ├── weights/
            │   └── best.pt        # 最优权重
            ├── reports/
            │   └── eval.json      # 评估报告
            └── export/
                └── model.onnx     # 导出模型
```

---

## 5. 关键数据模型

### 5.1 task.yaml

task.yaml 作为 Agent 上下文的一部分，包含完整的任务快照信息。超参详情见 `workspace/scripts/config.json`，评估详情见 `artifacts/reports/eval.json`，task.yaml 中保留摘要便于 Agent 快速了解全貌。

```yaml
task_id: task-20250214-001
status: completed            # pending | running | completed | failed | cancelled

# ── 用户输入 ──
task: "基于 Qwen2.5-7B 做情感分类微调，使用 LoRA，3 分类"
dataset: /mindbox/datasets/reviews.jsonl

# ── Agent 填写 ──
matched_skill: text-classification     # 匹配的任务类型 SKILL
framework: transformers                # 使用的训练框架
base_model: Qwen/Qwen2.5-7B           # 基座模型

hardware:
  gpu_count: 1
  gpu_type: "NVIDIA A100 80GB"

hyperparameters:                       # 关键超参摘要
  epochs: 3
  batch_size: 8
  learning_rate: 2e-5
  optimizer: AdamW
  lora_rank: 16
  lora_alpha: 32

results:                               # 评估结果摘要
  primary_metric:
    name: accuracy
    value: 0.92
  metrics:
    f1: 0.91
    precision: 0.93
    recall: 0.90

# ── 时间戳 ──
started_at: "2025-02-14T10:05:00Z"
completed_at: "2025-02-14T11:30:00Z"
duration_seconds: 5100
```

---

## 6. Task 状态机

```
                  ┌──────────────┐
       创建       │   pending    │
     ────────▶   │  (等待调度)   │
                  └──────┬───────┘
                         │ kernel 开始执行
                         ▼
                  ┌──────────────┐
                  │   running    │
                  │  (执行中)    │◀──── 自动重试（如 OOM 调参后）
                  └──┬───┬───┬──┘
                     │   │   │
           成功完成 ──┘   │   └── 用户取消
                         │
                         ▼
                  ┌──────────────┐
                  │   failed     │
                  │  (失败)      │
                  └──────────────┘
                  
           ┌──────────────┐    ┌──────────────┐
           │  completed   │    │  cancelled   │
           │  (完成)      │    │  (已取消)    │
           └──────────────┘    └──────────────┘
```

状态转移规则：

- `pending → running`：kernel 接管后
- `running → completed`：训练 + 评估全部成功
- `running → failed`：不可恢复的错误（如数据格式错误、超出最大重试次数）
- `running → running`：kernel 自动修复后重试（如 OOM 降低 batch size）
- `running → cancelled`：用户主动取消
- `failed → pending`：用户通过 retry 接口重试（在当前 task 基础上重新执行，如需全新执行应创建新 task）

---

## 7. API 设计（mindbox-server）

采用 RESTful 风格，JSON 格式。

### 7.1 任务管理

| 方法 | 路径 | 说明 |
|------|------|------|
| POST | `/api/v1/tasks` | 创建并启动 task |
| GET | `/api/v1/tasks` | 列出所有 task |
| GET | `/api/v1/tasks/{task_id}` | 获取 task 详情 |
| POST | `/api/v1/tasks/{task_id}/cancel` | 取消运行中的 task |
| POST | `/api/v1/tasks/{task_id}/retry` | 重试失败的 task |

### 7.2 日志

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/v1/tasks/{task_id}/logs` | 获取日志（支持 `?follow=true` SSE 流式） |

### 7.3 Artifact 管理

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/v1/tasks/{task_id}/artifacts` | 列出 artifact |
| GET | `/api/v1/tasks/{task_id}/artifacts/{path}` | 下载 artifact 文件 |

### 7.4 系统状态

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/health` | 简单健康检查 |
| GET | `/api/v1/status` | 系统详细状态（当前 task、GPU 信息、Kernel 类型） |

---

## 8. Kernel 工作流

Kernel 是整个系统的核心智能体，负责将用户任务描述转化为可执行的微调流程。

### 8.1 执行阶段

Kernel（Coding Agent）**全权驱动**整个微调流程。Agent 接收用户任务描述、数据集元信息和匹配的 SKILL.md 作为上下文，自主决定执行步骤，不存在固定的阶段划分或流水线。Agent 可以自由地：

- 选择合适的训练框架和模型
- 生成任意数量和结构的脚本
- 安装所需依赖
- 决定数据处理、训练、评估的具体方式
- 根据运行时情况动态调整策略（如 OOM 时降低 batch size 后重试）

**约束**：Agent 生成的训练脚本和配置文件**必须**放到 `workspace/scripts/` 目录下，训练执行的 cwd 为 `workspace/`。

为避免训练脚本的大量 stdout/stderr 直接进入 Agent 上下文窗口，Kernel 在启动 Claude Code 前会在 task 目录下生成 `AGENTS.md`（包含脚本执行与日志诊断规则），并创建 `CLAUDE.md -> AGENTS.md` 软链接。Claude Code 会自动读取 `CLAUDE.md` 并遵循其中约束，因此脚本输出**必须**重定向到 `logs/` 下的日志文件，再通过读取日志尾部进行诊断与结果提取。

**典型示例**（仅供参考，Agent 不受此限制）：

```
输入：用户 task + 数据集元信息 + 匹配的 SKILL.md
        │
        ▼
   Agent 分析任务 → 匹配 SKILL → 制定方案
        │
        ▼
   生成脚本到 workspace/scripts/（典型产出示例）：
     NLP 微调: data_prep.py, train.py, eval.py, config.json
     CV 检测:  prepare_data.py, train.py (框架内置评估)
        │
        ▼
   执行脚本 → 监控训练 → 异常处理 → 收集结果
        │
        ▼
   整理产物到 artifacts/ → 更新 task.yaml → 状态 → completed
```

### 8.2 Kernel 与 Server 的交互

Kernel 通过内部回调接口向 Server 报告进度：

```
Kernel ──▶ Server
  │
  ├── status_update(task_id, status, message)
  ├── log(task_id, level, message)
  ├── metric(task_id, step, metrics_dict)
  └── error(task_id, error_type, detail, recoverable)
```

Server 可以向 Kernel 发送控制信号：

```
Server ──▶ Kernel
  │
  ├── cancel(task_id)
  └── adjust(task_id, params)   # 预留：运行时调参
```

---

## 9. 容器设计

### 9.1 镜像层级

```
nvidia/cuda:12.1-runtime          # GPU 基础运行时
└── mindbox-base                   # Python + PyTorch + HuggingFace + 常用库
    └── mindbox                    # mindbox-server + mindbox-kernel + skills
```

### 9.2 环境变量

| 变量 | 必填 | 说明 |
|------|------|------|
| `MINDBOX_KERNEL` | 是 | Kernel 引擎类型：`claude-code` 或 `codex` |
| `ANTHROPIC_API_KEY` | 视 Kernel 而定 | Claude Code 所需的 API Key |
| `OPENAI_API_KEY` | 视 Kernel 而定 | Codex 所需的 API Key |

### 9.3 端口与挂载

| 端口 | 用途 |
|------|------|
| 8080 | mindbox-server API（HTTP + SSE） |
| 6006 | TensorBoard（可选） |

| 挂载点 | 用途 |
|--------|------|
| `/mindbox/datasets` | 数据集目录（只读，用户自行管理） |
| `/mindbox/tasks` | task 持久化 |
| `/mindbox/models` | 模型缓存（HuggingFace cache） |
| `/mindbox/skills` | 微调技能库（可扩展，用户可挂载自定义 Skill） |

### 9.4 启动流程

`mindbox sandbox start` 在底层执行等效的 Docker 命令：

```bash
docker run -d --gpus all \
  -p 8080:8080 -p 6006:6006 \
  -v $(pwd)/data:/mindbox/datasets:ro \
  -v $(pwd)/tasks:/mindbox/tasks \
  -v $(pwd)/mindbox-skills:/mindbox/skills \
  -v ~/.cache/huggingface:/mindbox/models \
  -e MINDBOX_KERNEL=claude-code \
  -e ANTHROPIC_API_KEY=sk-xxx \
  mindbox:latest
```

---

## 10. 项目结构

```
mindbox/
├── Cargo.toml                          # Workspace 根配置
├── Dockerfile                          # 多阶段构建镜像
├── docker-compose.yml                  # 本地开发 / 快速启动
├── README.md
├── LICENSE
│
├── mindbox-cli/                        # 用户侧命令行工具
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs                     # 入口
│       ├── commands/                   # 子命令实现
│       │   ├── mod.rs
│       │   ├── sandbox.rs              # sandbox start / stop / destroy
│       │   └── task.rs                 # task create / stop / attach / list
│       ├── client.rs                   # HTTP / SSE 客户端
│       └── ui.rs                       # ratatui 实时交互界面
│
├── mindbox-server/                     # 容器内 API 服务
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs                     # 入口，axum 路由注册
│       ├── routes/                     # API 路由层
│       │   ├── mod.rs
│       │   ├── tasks.rs                # /api/v1/tasks
│       │   ├── logs.rs                 # 日志流 (SSE)
│       │   ├── artifacts.rs            # Artifact 下载与导出
│       │   └── status.rs               # /health + /api/v1/status
│       ├── services/                   # 业务逻辑层
│       │   ├── mod.rs
│       │   ├── task_service.rs         # 任务调度与状态机
│       │   └── task_lock.rs            # 全局任务锁
│       └── error.rs                    # 统一错误处理
│
├── mindbox-kernel/                     # AI 推理内核
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs                      # Kernel trait 定义
│       ├── claude_code.rs              # Claude Code 引擎实现
│       ├── codex.rs                    # Codex 引擎实现
│       └── callback.rs                 # 向 Server 报告进度的回调接口
│
├── mindbox-common/                     # 共享类型与工具
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── config.rs                   # 环境变量 / 配置解析
│       └── types.rs                    # 跨 crate 共享的类型定义
│
├── mindbox-skills/                             # 微调技能库 (SKILL.md 集合)
│   ├── object-detection/SKILL.md       # 任务类型：目标检测
│   ├── image-classification/SKILL.md   # 任务类型：图像分类
│   ├── instance-segmentation/SKILL.md  # 任务类型：实例分割
│   ├── text-classification/SKILL.md    # 任务类型：文本分类
│   ├── seq2seq/SKILL.md                # 任务类型：序列到序列
│   ├── rlhf/SKILL.md                   # 任务类型：对齐训练
│   ├── lora/SKILL.md                   # 微调方法：LoRA/QLoRA
│   └── full-ft/SKILL.md               # 微调方法：全量微调
│
└── docs/                               # 文档
    └── mindbox-system-design.md
```

---

## 附录 A：技术栈选型（建议）

| 层级 | 技术 | 说明 |
|------|------|------|
| CLI | Rust (clap + ratatui) | 单二进制分发，clap 处理命令解析，ratatui 用于 `task attach` 等实时交互界面 |
| Server | Rust (axum / tokio) | 高性能异步服务，资源占用低 |
| Kernel | Claude Code SDK / Codex API | 按 skill 构造 prompt |
| 训练框架 | PyTorch + HuggingFace Transformers | 主流生态 |
| 微调方法 | PEFT (LoRA/QLoRA) / TRL | HuggingFace 官方库 |
| 模型导出 | ONNX Runtime / vLLM | 推理优化 |
| 日志 | tracing + JSONL | 结构化日志 |
| 监控 | TensorBoard | 训练可视化 |
| 容器 | Docker + NVIDIA Container Toolkit | GPU 支持 |
