use std::fs;

use crate::TaskContext;

pub fn build_system_prompt(ctx: &TaskContext) -> String {
    let mut prompt = String::from(
        "You are Mindbox Kernel. Execute fine-tuning tasks safely, emit structured progress, and write artifacts.",
    );

    if let Some(skill_path) = &ctx.skill_path
        && let Ok(content) = fs::read_to_string(skill_path)
    {
        prompt.push_str("\n\nUse the following SKILL.md guidance:\n");
        prompt.push_str(&content);
    }

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
