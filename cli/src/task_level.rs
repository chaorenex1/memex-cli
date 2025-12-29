use crate::commands::cli::TaskLevel;

pub fn infer_task_level(prompt: &str) -> TaskLevel {
    let s = prompt.trim();
    if s.is_empty() {
        return TaskLevel::L1;
    }

    let lower = s.to_ascii_lowercase();

    // Strong engineering / multi-step signals => L2
    if lower.contains("architecture")
        || lower.contains("debug")
        || lower.contains("refactor")
        || lower.contains("compile")
        || lower.contains("cargo")
        || lower.contains("stack trace")
        || lower.contains("benchmark")
        || s.contains("```")
    {
        return TaskLevel::L2;
    }

    // High creativity / style-heavy signals => L3
    if lower.contains("story")
        || lower.contains("novel")
        || lower.contains("brand")
        || lower.contains("marketing")
        || lower.contains("style")
    {
        return TaskLevel::L3;
    }

    // Very short tool-like requests => L0
    if s.chars().count() <= 200
        && (lower.contains("translate")
            || lower.contains("format")
            || lower.contains("json")
            || lower.contains("rewrite"))
    {
        return TaskLevel::L0;
    }

    TaskLevel::L1
}
