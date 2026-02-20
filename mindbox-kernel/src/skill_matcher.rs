use std::path::{Path, PathBuf};

pub fn match_skill(task_description: &str, skills_dir: &Path) -> Option<PathBuf> {
    let desc = task_description.to_lowercase();

    let candidates: &[(&[&str], &str)] = &[
        (
            &["object detection", "检测", "yolo", "rtdetr", "faster r-cnn"],
            "object-detection",
        ),
        (
            &["classification", "分类", "text classification", "sentiment"],
            "text-classification",
        ),
        (
            &["segmentation", "分割", "mask r-cnn", "yolo-seg"],
            "instance-segmentation",
        ),
        (
            &["translation", "summarization", "seq2seq", "问答"],
            "seq2seq",
        ),
        (&["rlhf", "alignment", "对齐"], "rlhf"),
        (&["lora", "qlora"], "lora"),
        (&["full fine-tune", "full-ft", "全量微调"], "full-ft"),
    ];

    for (keywords, folder) in candidates {
        if keywords.iter().any(|k| desc.contains(k)) {
            let path = skills_dir.join(folder).join("SKILL.md");
            if path.exists() {
                return Some(path);
            }
        }
    }

    find_first_skill(skills_dir)
}

fn find_first_skill(root: &Path) -> Option<PathBuf> {
    let entries = std::fs::read_dir(root).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let candidate = path.join("SKILL.md");
            if candidate.exists() {
                return Some(candidate);
            }
            if let Some(found) = find_first_skill(&path) {
                return Some(found);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_none_for_missing_dir() {
        let missing = PathBuf::from("/tmp/not-exists-mindbox-skill");
        assert!(match_skill("hello", &missing).is_none());
    }
}
