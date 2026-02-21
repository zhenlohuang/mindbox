use crate::TaskContext;

pub fn build_system_prompt(_ctx: &TaskContext) -> String {
    let prompt = String::from(
        "You are Mindbox Kernel. Execute fine-tuning tasks safely, emit structured progress, and write artifacts.",
    );
    prompt
}

pub fn build_user_prompt(ctx: &TaskContext) -> String {
    format!(
        "Project: {}\nTask ID: {}\nTask: {}\nDataset: {}\nTask Dir: {}\nPlease perform the full fine-tuning workflow and report progress as JSON lines.",
        ctx.project.id,
        ctx.task.id,
        ctx.task.task_description,
        ctx.dataset.dataset_path.display(),
        ctx.task_dir.display()
    )
}
