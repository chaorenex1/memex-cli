//! Structured Prompt Builder
//!
//! Builds a structured prompt from role prompt, content, and files

use std::collections::BTreeSet;

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
        let content = self.content.trim();

        // 1. Role setting (if provided)
        if let Some(ref role) = self.role_prompt {
            if !role.trim().is_empty() {
                sections.push(format!("<Role>\n{}\n</Role>", role.trim()));
            }
        }

        // 2. Task scope (files)
        if !self.files.is_empty() {
            sections.push(format!("<Scope>\n{}\n</Scope>", self.format_scope()));
        }

        // 3. Task steps (main content)
        sections.push(format!("<TaskSteps>\n{}\n</TaskSteps>", content));

        sections.join("\n\n")
    }

    fn format_scope(&self) -> String {
        if self.files.is_empty() {
            return "No specific scope".to_string();
        }

        let files = self.unique_files();

        // Use user-provided scope if available
        if let Some(ref scope) = self.scope {
            if !scope.trim().is_empty() {
                return format!("{}\n\nRelevant files:\n{}", scope.trim(), self.format_file_list(&files));
            }
        }

        self.format_file_list(&files)
    }

    fn unique_files(&self) -> Vec<String> {
        let mut seen = BTreeSet::new();

        self.files
            .iter()
            .map(|file| file.trim())
            .filter(|file| !file.is_empty())
            .filter(|file| seen.insert((*file).to_string()))
            .map(ToOwned::to_owned)
            .collect()
    }

    fn format_file_list(&self, files: &[String]) -> String {
        if files.is_empty() {
            return "No specific scope".to_string();
        }

        files
            .iter()
            .map(|file| format!("- {}", file))
            .collect::<Vec<_>>()
            .join("\n")
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

        assert!(prompt.contains("<Role>"));
        assert!(prompt.contains("<Scope>"));
        assert!(prompt.contains("<TaskSteps>"));
        assert!(prompt.contains("你是一个代码审查专家"));
        assert!(prompt.contains("./src/"));
        assert!(prompt.contains("请分析代码结构"));
    }

    #[test]
    fn test_structured_prompt_builder_empty() {
        let prompt = StructuredPromptBuilder::new()
            .content("简单任务".to_string())
            .build();

        assert!(!prompt.contains("<Role>"));
        assert!(!prompt.contains("<Scope>"));
        assert!(prompt.contains("<TaskSteps>"));
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

        assert!(prompt.contains("<Scope>"));
        assert!(prompt.contains("仅处理 Rust 文件"));
        assert!(prompt.contains("Relevant files:"));
        assert!(prompt.contains("./src/a.rs"));
        assert!(prompt.contains("./tests/c.rs"));
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

        assert!(prompt.contains("./src/a.rs"));
        assert!(prompt.contains("./src/b.rs"));
        assert!(prompt.contains("./tests/c.rs"));
    }

    #[test]
    fn test_structured_prompt_builder_deduplicates_files() {
        let prompt = StructuredPromptBuilder::new()
            .content("请只处理给定文件".to_string())
            .files(vec!["./src/a.rs".to_string(), "./src/a.rs".to_string()])
            .build();

        assert_eq!(prompt.matches("./src/a.rs").count(), 1);
    }
}
