//! Integration tests for structured text input parsing and execution
//!
//! These tests verify the end-to-end flow from raw input to task execution.

use std::fs;
use tempfile::TempDir;

/// Helper to create test input files
fn setup_test_dir() -> TempDir {
    TempDir::new().expect("Failed to create temp dir")
}

/// Test: Plain text mode wraps content as single task
#[test]
fn test_plain_text_single_task() {
    use memex_core::api::InputParser;

    let input = "编写一个快速排序算法的 Rust 实现";

    let tasks = InputParser::parse(
        input, false, // structured = false
        "codex", "/tmp", None, "text",
    )
    .expect("Failed to parse plain text");

    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].content, input);
    assert_eq!(tasks[0].backend, "codex");
    assert_eq!(tasks[0].workdir, "/tmp");
    assert!(tasks[0].id.starts_with("task-"));
    assert!(tasks[0].dependencies.is_empty());
}

/// Test: Structured mode parses single task correctly
#[test]
fn test_structured_single_task() {
    use memex_core::api::InputParser;

    let input = r#"
---TASK---
id: test-task
backend: claude
workdir: /project
model: claude-sonnet-4
---CONTENT---
分析这段代码的性能问题
---END---
"#;

    let tasks = InputParser::parse(
        input,
        true, // structured = true
        "default-backend",
        "/default",
        None,
        "text",
    )
    .expect("Failed to parse structured text");

    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].id, "test-task");
    assert_eq!(tasks[0].backend, "claude");
    assert_eq!(tasks[0].model, Some("claude-sonnet-4".to_string()));
    assert_eq!(tasks[0].content.trim(), "分析这段代码的性能问题");
}

/// Test: Structured mode parses multiple tasks with dependencies
#[test]
fn test_structured_multi_task_dependencies() {
    use memex_core::api::InputParser;

    let input = r#"
---TASK---
id: design
backend: claude
workdir: /project
---CONTENT---
设计 API 接口
---END---

---TASK---
id: implement
backend: codex
workdir: /project
dependencies: design
---CONTENT---
实现 API 代码
---END---

---TASK---
id: test
backend: codex
workdir: /project
dependencies: implement
---CONTENT---
编写测试
---END---
"#;

    let tasks = InputParser::parse(input, true, "codex", "/tmp", None, "text")
        .expect("Failed to parse multi-task");

    assert_eq!(tasks.len(), 3);

    // Task 1: design (no dependencies)
    assert_eq!(tasks[0].id, "design");
    assert!(tasks[0].dependencies.is_empty());

    // Task 2: implement (depends on design)
    assert_eq!(tasks[1].id, "implement");
    assert_eq!(tasks[1].dependencies, vec!["design"]);

    // Task 3: test (depends on implement)
    assert_eq!(tasks[2].id, "test");
    assert_eq!(tasks[2].dependencies, vec!["implement"]);
}

/// Test: Error handling for invalid structured input
#[test]
fn test_structured_parse_error_helpful() {
    use memex_core::api::InputParser;

    let input = "just plain text without markers";

    let result = InputParser::parse(input, true, "codex", "/tmp", None, "text");

    assert!(result.is_err());
    let err_msg = result.unwrap_err();

    // Should suggest --no-structured-text
    assert!(err_msg.contains("Failed to parse structured text"));
    assert!(err_msg.contains("--no-structured-text"));
}

/// Test: Circular dependency detection
#[test]
fn test_circular_dependency_error() {
    use memex_core::api::InputParser;

    let input = r#"
---TASK---
id: task1
backend: codex
workdir: /tmp
dependencies: task2
---CONTENT---
Step 1
---END---

---TASK---
id: task2
backend: codex
workdir: /tmp
dependencies: task1
---CONTENT---
Step 2
---END---
"#;

    let result = InputParser::parse(input, true, "codex", "/tmp", None, "text");

    assert!(result.is_err());
    let err_msg = result.unwrap_err();
    // Error message wrapped by InputParser
    assert!(
        err_msg.contains("Circular dependency") || err_msg.contains("circular"),
        "Expected circular dependency error, got: {}",
        err_msg
    );
}

/// Test: Missing required field error
#[test]
fn test_missing_required_field() {
    use memex_core::api::InputParser;

    let input = r#"
---TASK---
backend: codex
---CONTENT---
test content
---END---
"#;

    let result = InputParser::parse(input, true, "codex", "/tmp", None, "text");

    assert!(result.is_err());
    let err_msg = result.unwrap_err();
    // Error message wrapped by InputParser with "Failed to parse structured text:" prefix
    assert!(
        err_msg.contains("metadata missing required field"),
        "Expected missing field error, got: {}",
        err_msg
    );
}

/// Test: File-based input parsing (simulates --prompt-file)
#[test]
fn test_file_based_input() {
    use memex_core::api::InputParser;

    let temp_dir = setup_test_dir();
    let file_path = temp_dir.path().join("test_input.txt");

    let content = r#"
---TASK---
id: file-task
backend: codex
workdir: /project
---CONTENT---
从文件读取的任务
---END---
"#;

    fs::write(&file_path, content).expect("Failed to write test file");

    // Read and parse
    let input = fs::read_to_string(&file_path).expect("Failed to read file");
    let tasks = InputParser::parse(&input, true, "codex", "/tmp", None, "text")
        .expect("Failed to parse file input");

    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].id, "file-task");
    assert_eq!(tasks[0].content.trim(), "从文件读取的任务");
}

/// Test: Plain text mode with model override
#[test]
fn test_plain_text_with_model() {
    use memex_core::api::InputParser;

    let input = "使用特定模型的提示词";

    let tasks = InputParser::parse(
        input,
        false,
        "claude",
        "/tmp",
        Some("claude-opus-4".to_string()),
        "jsonl",
    )
    .expect("Failed to parse");

    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].backend, "claude");
    assert_eq!(tasks[0].model, Some("claude-opus-4".to_string()));
    assert_eq!(tasks[0].stream_format, "jsonl");
}

/// Test: Auto-generated task IDs are unique
#[test]
fn test_auto_generated_task_ids_unique() {
    use memex_core::api::InputParser;
    use std::collections::HashSet;

    let mut ids = HashSet::new();

    for i in 0..10 {
        let input = format!("Task {}", i);
        let tasks = InputParser::parse(&input, false, "codex", "/tmp", None, "text")
            .expect("Failed to parse");

        assert_eq!(tasks.len(), 1);
        let id = tasks[0].id.clone();

        // ID should be unique
        assert!(ids.insert(id.clone()), "Duplicate ID: {}", id);

        // ID format: task-YYYYMMDDHHmmss-XXXX
        assert!(id.starts_with("task-"));
        assert!(id.len() >= 24);
    }
}

/// Test: Multiline content preservation
#[test]
fn test_multiline_content_preserved() {
    use memex_core::api::InputParser;

    let input = r#"
---TASK---
id: multiline
backend: codex
workdir: /tmp
---CONTENT---
第一行
第二行
第三行
---END---
"#;

    let tasks =
        InputParser::parse(input, true, "codex", "/tmp", None, "text").expect("Failed to parse");

    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].content.trim(), "第一行\n第二行\n第三行");
}

/// Test: Default stream format fallback
#[test]
fn test_default_stream_format() {
    use memex_core::api::InputParser;

    // Plain text mode uses provided default
    let tasks = InputParser::parse(
        "test", false, "codex", "/tmp", None, "jsonl", // custom format
    )
    .expect("Failed to parse");

    assert_eq!(tasks[0].stream_format, "jsonl");

    // Structured mode can override per-task
    let input = r#"
---TASK---
id: test
backend: codex
workdir: /tmp
stream-format: text
---CONTENT---
test
---END---
"#;

    let tasks2 =
        InputParser::parse(input, true, "codex", "/tmp", None, "jsonl").expect("Failed to parse");

    assert_eq!(tasks2[0].stream_format, "text"); // From task definition
}
