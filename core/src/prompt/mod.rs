//! Structured Prompt Builder
//!
//! Builds a structured prompt from role prompt, content, and files

use std::collections::HashMap;

/// Structured prompt builder
pub struct StructuredPromptBuilder {
    role_prompt: Option<String>,
    content: String,
    files: Vec<String>,
    scope: Option<String>,
}

impl StructuredPromptBuilder {
    pub fn new() -> Self {
        Self {
            role_prompt: None,
            content: String::new(),
            files: Vec::new(),
            scope: None,
        }
    }

    pub fn role_prompt(mut self, role_prompt: Option<String>) -> Self {
        self.role_prompt = role_prompt;
        self
    }

    pub fn content(mut self, content: String) -> Self {
        self.content = content;
        self
    }

    pub fn files(mut self, files: Vec<String>) -> Self {
        self.files = files;
        self
    }

    pub fn scope(mut self, scope: Option<String>) -> Self {
        self.scope = scope;
        self
    }

    /// Build the structured prompt
    pub fn build(self) -> String {
        let mut sections = Vec::new();

        // 1. Role setting (if provided)
        if let Some(ref role) = self.role_prompt {
            if !role.trim().is_empty() {
                sections.push(format!("## Role\n\n{}", role.trim()));
            }
        }

        // 2. Task scope (files)
        if !self.files.is_empty() {
            sections.push(format!("## Scope\n\n{}", self.format_scope()));
        }

        // 3. Task steps (main content)
        sections.push(format!("## Steps\n\n{}", self.content));

        sections.join("\n\n")
    }

    fn format_scope(&self) -> String {
        if self.files.is_empty() {
            return "No specific scope".to_string();
        }

        // Use user-provided scope if available
        if let Some(ref scope) = self.scope {
            if !scope.trim().is_empty() {
                return scope.clone();
            }
        }

        // Group files by directory
        let mut dirs: HashMap<String, Vec<String>> = HashMap::new();
        let mut standalone_files: Vec<String> = Vec::new();

        for file in &self.files {
            if let Some(pos) = file.rfind('/') {
                let dir = &file[..pos];
                dirs.entry(dir.to_string())
                    .or_default()
                    .push(file.clone());
            } else {
                standalone_files.push(file.clone());
            }
        }

        let mut result = Vec::new();

        // Add directories
        let mut dir_keys: Vec<_> = dirs.keys().collect();
        dir_keys.sort();
        for dir in dir_keys {
            result.push(format!("- `{}/`", dir));
        }

        // Add standalone files
        for file in standalone_files {
            result.push(format!("- `{}`", file));
        }

        if result.is_empty() {
            "No specific scope".to_string()
        } else {
            result.join("\n")
        }
    }
}

impl Default for StructuredPromptBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_structured_prompt_builder_basic() {
        let prompt = StructuredPromptBuilder::new()
            .role_prompt(Some("你是一个代码审查专家".to_string()))
            .content("请分析代码结构".to_string())
            .files(vec!["./src/main.rs".to_string()])
            .build();

        assert!(prompt.contains("## Role"));
        assert!(prompt.contains("## Scope"));
        assert!(prompt.contains("## Steps"));
        assert!(prompt.contains("你是一个代码审查专家"));
        assert!(prompt.contains("./src/"));
        assert!(prompt.contains("请分析代码结构"));
    }

    #[test]
    fn test_structured_prompt_builder_empty() {
        let prompt = StructuredPromptBuilder::new()
            .content("简单任务".to_string())
            .build();

        assert!(!prompt.contains("## Role"));
        assert!(!prompt.contains("## Scope"));
        assert!(prompt.contains("## Steps"));
        assert!(prompt.contains("简单任务"));
    }

    #[test]
    fn test_structured_prompt_builder_with_scope() {
        let prompt = StructuredPromptBuilder::new()
            .content("请处理这些文件".to_string())
            .files(vec![
                "./src/a.rs".to_string(),
                "./src/b.rs".to_string(),
                "./tests/c.rs".to_string(),
            ])
            .scope(Some("仅处理 Rust 文件".to_string()))
            .build();

        assert!(prompt.contains("## Scope"));
        assert!(prompt.contains("仅处理 Rust 文件"));
    }

    #[test]
    fn test_structured_prompt_builder_with_multiple_dirs() {
        let prompt = StructuredPromptBuilder::new()
            .content("请处理这些文件".to_string())
            .files(vec![
                "./src/a.rs".to_string(),
                "./src/b.rs".to_string(),
                "./tests/c.rs".to_string(),
            ])
            .build();

        assert!(prompt.contains("./src/"));
        assert!(prompt.contains("./tests/"));
    }
}
