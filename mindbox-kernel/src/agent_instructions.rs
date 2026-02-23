use std::path::Path;

use anyhow::Result;
use tokio::fs;

pub fn build_agent_md() -> &'static str {
    include_str!("kernel_agent.md")
}

pub async fn ensure_agent_files(
    task_dir: &Path,
    agent_md: &str,
    skills_dir: Option<&Path>,
) -> Result<()> {
    fs::write(task_dir.join("AGENT.md"), agent_md).await?;

    let claude_md_path = task_dir.join("CLAUDE.md");
    if fs::symlink_metadata(&claude_md_path).await.is_err() {
        fs::symlink("AGENT.md", &claude_md_path).await?;
    }

    ensure_task_skills_link(task_dir, skills_dir).await?;

    Ok(())
}

async fn ensure_task_skills_link(task_dir: &Path, skills_dir: Option<&Path>) -> Result<()> {
    let Some(skills_dir) = skills_dir else {
        return Ok(());
    };

    let Ok(metadata) = fs::symlink_metadata(skills_dir).await else {
        return Ok(());
    };
    if !metadata.file_type().is_dir() {
        return Ok(());
    }

    let claude_dir = task_dir.join(".claude");
    fs::create_dir_all(&claude_dir).await?;

    let task_skills_path = claude_dir.join("skills");
    if let Ok(existing) = fs::symlink_metadata(&task_skills_path).await {
        if existing.file_type().is_symlink() {
            fs::remove_file(&task_skills_path).await?;
        } else {
            // Do not clobber non-symlink paths created by users/tools.
            return Ok(());
        }
    }

    fs::symlink(skills_dir, &task_skills_path).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    fn unique_temp_dir(prefix: &str) -> std::path::PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock drift")
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}-{suffix}"))
    }

    #[tokio::test]
    async fn ensure_agent_files_creates_agent_and_symlink() {
        let task_dir = unique_temp_dir("mindbox-agent-files");
        fs::create_dir_all(&task_dir)
            .await
            .expect("create temp dir");

        let content = "# Mindbox Kernel Agent\nRule";
        ensure_agent_files(&task_dir, content, None)
            .await
            .expect("write agent files");

        let agent = fs::read_to_string(task_dir.join("AGENT.md"))
            .await
            .expect("read AGENT.md");
        assert_eq!(agent, content);

        let claude = fs::read_to_string(task_dir.join("CLAUDE.md"))
            .await
            .expect("read CLAUDE.md");
        assert_eq!(claude, content);

        let metadata = fs::symlink_metadata(task_dir.join("CLAUDE.md"))
            .await
            .expect("symlink metadata");
        assert!(metadata.file_type().is_symlink());

        fs::remove_file(task_dir.join("CLAUDE.md"))
            .await
            .expect("remove CLAUDE.md");
        fs::remove_file(task_dir.join("AGENT.md"))
            .await
            .expect("remove AGENT.md");
        fs::remove_dir_all(&task_dir)
            .await
            .expect("remove temp dir");
    }

    #[tokio::test]
    async fn ensure_agent_files_links_task_scoped_claude_skills() {
        let root_dir = unique_temp_dir("mindbox-agent-files-skills");
        let task_dir = root_dir.join("task");
        let skills_dir = root_dir.join("skills");
        fs::create_dir_all(&task_dir)
            .await
            .expect("create task temp dir");
        fs::create_dir_all(&skills_dir)
            .await
            .expect("create skills temp dir");

        ensure_agent_files(&task_dir, "# Agent", Some(&skills_dir))
            .await
            .expect("write agent files with skills");
        ensure_agent_files(&task_dir, "# Agent", Some(&skills_dir))
            .await
            .expect("rerun agent file setup");

        let task_skills_path = task_dir.join(".claude").join("skills");
        let metadata = fs::symlink_metadata(&task_skills_path)
            .await
            .expect("symlink metadata");
        assert!(metadata.file_type().is_symlink());

        let target = fs::read_link(&task_skills_path).await.expect("read link");
        assert_eq!(target, skills_dir);

        fs::remove_dir_all(&root_dir)
            .await
            .expect("remove temp dir");
    }

    #[tokio::test]
    async fn ensure_agent_files_skips_missing_skills_dir() {
        let root_dir = unique_temp_dir("mindbox-agent-files-missing-skills");
        let task_dir = root_dir.join("task");
        let missing_skills_dir = root_dir.join("missing-skills");
        fs::create_dir_all(&task_dir)
            .await
            .expect("create task temp dir");

        ensure_agent_files(&task_dir, "# Agent", Some(&missing_skills_dir))
            .await
            .expect("write agent files without skills dir");

        assert!(
            fs::symlink_metadata(task_dir.join(".claude").join("skills"))
                .await
                .is_err()
        );

        fs::remove_dir_all(&root_dir)
            .await
            .expect("remove temp dir");
    }
}
