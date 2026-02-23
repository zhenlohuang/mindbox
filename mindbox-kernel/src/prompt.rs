use crate::TaskContext;

pub fn build_user_prompt(ctx: &TaskContext) -> String {
    format!(
        "Task ID: {}\nTask: {}\nDataset: {}\nTask Dir: {}\nPlease perform the full fine-tuning workflow",
        ctx.task.id,
        ctx.task.task_description,
        ctx.dataset.dataset_path.display(),
        ctx.task_dir.display()
    )
}
