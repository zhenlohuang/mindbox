# Mindbox Kernel Agent

## Environment

- Datasets root: /mindbox/datasets (read-only, managed by user)
- Base model weights root: /mindbox/models (HuggingFace cache)
- Task working directory: the current directory (a task-specific directory under /mindbox/tasks/{task_id}/)

## Task Working Directory Layout

The task working directory has the following structure. Create subdirectories as needed during execution.

```
{task_dir}/
├── workspace/             # Agent writable workspace (scripts run with cwd=workspace/)
│   ├── scripts/           # Generated training scripts and configs (MUST place scripts here)
│   └── tb_logs/           # TensorBoard logs
├── logs/
│   ├── kernel.log         # Kernel process log (auto-managed)
│   ├── train.log          # Training script output (Agent redirects here)
│   └── ...                # Other script output logs
└── artifacts/             # Final outputs
    ├── weights/
    │   └── best.pt        # Best checkpoint weights
    ├── reports/
    │   └── eval.json      # Evaluation report
    ├── export/
    │   └── model.onnx     # Exported model
    └── tb_logs/           # Copied from workspace/tb_logs/ on completion
```

## Script Execution Rules

When running training, evaluation, or data preparation scripts:
1. ALWAYS redirect stdout and stderr to a log file under logs/. Example:
   mkdir -p logs && python workspace/scripts/train.py > logs/train.log 2>&1
2. NEVER run scripts with output directly to terminal - this wastes your context window.
3. After the script exits, check the exit code ($?).
4. If the script FAILED (non-zero exit), read the last 50 lines of the log file to diagnose the error.
5. If the script SUCCEEDED, read the last 20 lines to extract final metrics/results.
