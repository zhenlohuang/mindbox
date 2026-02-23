use std::path::Path;

use anyhow::Result;
use tokio::fs;

pub fn build_agent_md() -> &'static str {
    include_str!("kernel_agent.md")
}

pub async fn ensure_agent_files(task_dir: &Path, agent_md: &str) -> Result<()> {
    fs::write(task_dir.join("AGENT.md"), agent_md).await?;

    let claude_md_path = task_dir.join("CLAUDE.md");
    if fs::symlink_metadata(&claude_md_path).await.is_err() {
        fs::symlink("AGENT.md", &claude_md_path).await?;
    }

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
        ensure_agent_files(&task_dir, content)
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

}
