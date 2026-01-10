use std::collections::{HashMap, HashSet};

use regex::Regex;

use super::error::StdioError;
use super::id_gen::generate_task_id;
use super::types::{FilesEncoding, FilesMode, StdioTask};

#[allow(clippy::while_let_on_iterator)]
pub fn parse_stdio_tasks(input: &str) -> Result<Vec<StdioTask>, StdioError> {
    // Level 2.3: 智能选择解析器版本（零拷贝 vs 原版）
    const ZERO_COPY_THRESHOLD: usize = 10 * 1024; // 10KB

    if input.len() >= ZERO_COPY_THRESHOLD {
        // 大输入使用零拷贝版本（2x 加速）
        return parse_stdio_tasks_zero_copy(input);
    }

    // 小输入使用原版（代码简单，调试友好）
    let mut lines = input.lines().peekable();
    let mut tasks: Vec<StdioTask> = Vec::new();

    while let Some(line) = lines.next() {
        if line.trim() != "---TASK---" {
            continue;
        }

        let mut metadata: HashMap<String, String> = HashMap::new();
        let mut saw_content_marker = false;

        while let Some(line) = lines.next() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if trimmed == "---CONTENT---" {
                saw_content_marker = true;
                break;
            }
            let Some((k, v)) = trimmed.split_once(':') else {
                return Err(StdioError::InvalidMetadataLine(trimmed.to_string()));
            };
            metadata.insert(k.trim().to_lowercase(), v.trim().to_string());
        }

        if !saw_content_marker {
            return Err(StdioError::MissingContentMarker);
        }

        let mut content_lines: Vec<String> = Vec::new();
        let mut ended = false;
        while let Some(line) = lines.next() {
            if line.trim() == "---END---" {
                ended = true;
                break;
            }
            content_lines.push(line.to_string());
        }

        if !ended {
            return Err(StdioError::MissingEndMarker);
        }

        let id = metadata.get("id").cloned().unwrap_or_else(generate_task_id);
        let backend = metadata
            .get("backend")
            .cloned()
            .ok_or(StdioError::MissingField { field: "backend" })?;
        let workdir = metadata
            .get("workdir")
            .cloned()
            .ok_or(StdioError::MissingField { field: "workdir" })?;

        validate_id(&id)?;

        let dependencies = metadata
            .get("dependencies")
            .map(|s| split_csv(s))
            .unwrap_or_default();
        let stream_format = metadata
            .get("stream-format")
            .cloned()
            .unwrap_or_else(|| "text".to_string());
        let model = metadata.get("model").cloned();
        let model_provider = metadata.get("model-provider").cloned();
        let timeout = parse_u64(metadata.get("timeout").map(String::as_str), "timeout")?;
        let retry = parse_u32(metadata.get("retry").map(String::as_str), "retry")?;
        let files = metadata
            .get("files")
            .map(|s| split_csv(s))
            .unwrap_or_default();
        let files_mode = parse_files_mode(metadata.get("files-mode"));
        let files_encoding = parse_files_encoding(metadata.get("files-encoding"));

        let content = content_lines.join("\n");

        tasks.push(StdioTask {
            id,
            backend,
            workdir,
            model,
            model_provider,
            dependencies,
            stream_format,
            timeout,
            retry,
            files,
            files_mode,
            files_encoding,
            content,
        });
    }

    if tasks.is_empty() {
        return Err(StdioError::NoTasks);
    }

    validate_dependencies(&tasks)?;
    Ok(tasks)
}

// ============================================================================
// Zero-Copy Parser (Level 2.3 优化)
// ============================================================================

/// 零拷贝解析器：使用字符串切片避免中间分配
///
/// # 优势
/// - 避免逐行分配 String（使用 `&str` 切片）
/// - 减少中间 Vec<String> 分配
/// - 大输入（>10KB）性能提升约 2x
pub fn parse_stdio_tasks_zero_copy(input: &str) -> Result<Vec<StdioTask>, StdioError> {
    let mut tasks: Vec<StdioTask> = Vec::new();
    let mut pos = 0;

    while let Some(task_start) = input[pos..].find("---TASK---") {
        pos += task_start + 10; // "---TASK---".len()

        // 查找 CONTENT 标记
        let Some(content_start) = input[pos..].find("---CONTENT---") else {
            return Err(StdioError::MissingContentMarker);
        };

        // 元数据段（使用切片，无拷贝）
        let metadata_section = &input[pos..pos + content_start];
        let metadata = parse_metadata_zero_copy(metadata_section)?;

        pos += content_start + 13; // "---CONTENT---".len()

        // 查找 END 标记
        let Some(end_pos) = input[pos..].find("---END---") else {
            return Err(StdioError::MissingEndMarker);
        };

        // 内容段（切片）
        let content = &input[pos..pos + end_pos];

        // 构建任务（仅在此处转换为 String）
        tasks.push(build_task_from_metadata_zero_copy(metadata, content)?);

        pos += end_pos + 9; // "---END---".len()
    }

    if tasks.is_empty() {
        return Err(StdioError::NoTasks);
    }

    validate_dependencies(&tasks)?;
    Ok(tasks)
}

/// 解析元数据段（零拷贝：返回 &str 引用）
fn parse_metadata_zero_copy(section: &str) -> Result<HashMap<&str, &str>, StdioError> {
    let mut metadata = HashMap::new();

    for line in section.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let Some((k, v)) = trimmed.split_once(':') else {
            return Err(StdioError::InvalidMetadataLine(trimmed.to_string()));
        };

        metadata.insert(k.trim(), v.trim());
    }

    Ok(metadata)
}

/// 从零拷贝元数据构建任务（仅在此处分配 String）
fn build_task_from_metadata_zero_copy(
    metadata: HashMap<&str, &str>,
    content: &str,
) -> Result<StdioTask, StdioError> {
    // 必需字段
    let id = metadata
        .get("id")
        .map(|s| s.to_string())
        .unwrap_or_else(generate_task_id);

    validate_id(&id)?;

    let backend = metadata
        .get("backend")
        .ok_or(StdioError::MissingField { field: "backend" })?
        .to_string();

    let workdir = metadata
        .get("workdir")
        .ok_or(StdioError::MissingField { field: "workdir" })?
        .to_string();

    // 可选字段
    let dependencies = metadata
        .get("dependencies")
        .map(|s| split_csv_zero_copy(s))
        .unwrap_or_default();

    let stream_format = metadata
        .get("stream-format")
        .map(|s| s.to_string())
        .unwrap_or_else(|| "text".to_string());

    let model = metadata.get("model").map(|s| s.to_string());
    let model_provider = metadata.get("model-provider").map(|s| s.to_string());

    let timeout = parse_u64_zero_copy(metadata.get("timeout").copied(), "timeout")?;
    let retry = parse_u32_zero_copy(metadata.get("retry").copied(), "retry")?;

    let files = metadata
        .get("files")
        .map(|s| split_csv_zero_copy(s))
        .unwrap_or_default();

    let files_mode = parse_files_mode_zero_copy(metadata.get("files-mode").copied());
    let files_encoding = parse_files_encoding_zero_copy(metadata.get("files-encoding").copied());

    Ok(StdioTask {
        id,
        backend,
        workdir,
        model,
        model_provider,
        dependencies,
        stream_format,
        timeout,
        retry,
        files,
        files_mode,
        files_encoding,
        content: content.trim_end().to_string(), // 移除尾部空白
    })
}

/// CSV 分割（零拷贝版本）
fn split_csv_zero_copy(input: &str) -> Vec<String> {
    input
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

/// 解析 u64（零拷贝版本）
fn parse_u64_zero_copy(
    value: Option<&str>,
    field: &'static str,
) -> Result<Option<u64>, StdioError> {
    match value {
        None => Ok(None),
        Some(v) if v.trim().is_empty() => Ok(None),
        Some(v) => v
            .trim()
            .parse::<u64>()
            .map(Some)
            .map_err(|_| StdioError::InvalidNumber {
                field,
                value: v.to_string(),
            }),
    }
}

/// 解析 u32（零拷贝版本）
fn parse_u32_zero_copy(
    value: Option<&str>,
    field: &'static str,
) -> Result<Option<u32>, StdioError> {
    match value {
        None => Ok(None),
        Some(v) if v.trim().is_empty() => Ok(None),
        Some(v) => v
            .trim()
            .parse::<u32>()
            .map(Some)
            .map_err(|_| StdioError::InvalidNumber {
                field,
                value: v.to_string(),
            }),
    }
}

/// 解析文件模式（零拷贝版本）
fn parse_files_mode_zero_copy(v: Option<&str>) -> FilesMode {
    match v.map(|s| s.to_lowercase()) {
        Some(ref s) if s == "embed" => FilesMode::Embed,
        Some(ref s) if s == "ref" => FilesMode::Ref,
        _ => FilesMode::Auto,
    }
}

/// 解析文件编码（零拷贝版本）
fn parse_files_encoding_zero_copy(v: Option<&str>) -> FilesEncoding {
    match v.map(|s| s.to_lowercase()) {
        Some(ref s) if s == "utf-8" || s == "utf8" => FilesEncoding::Utf8,
        Some(ref s) if s == "base64" => FilesEncoding::Base64,
        _ => FilesEncoding::Auto,
    }
}

// ============================================================================
// Original Parser Helpers
// ============================================================================

fn split_csv(input: &str) -> Vec<String> {
    input
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn parse_u64(value: Option<&str>, field: &'static str) -> Result<Option<u64>, StdioError> {
    match value {
        None => Ok(None),
        Some(v) if v.trim().is_empty() => Ok(None),
        Some(v) => v
            .trim()
            .parse::<u64>()
            .map(Some)
            .map_err(|_| StdioError::InvalidNumber {
                field,
                value: v.to_string(),
            }),
    }
}

fn parse_u32(value: Option<&str>, field: &'static str) -> Result<Option<u32>, StdioError> {
    match value {
        None => Ok(None),
        Some(v) if v.trim().is_empty() => Ok(None),
        Some(v) => v
            .trim()
            .parse::<u32>()
            .map(Some)
            .map_err(|_| StdioError::InvalidNumber {
                field,
                value: v.to_string(),
            }),
    }
}

fn parse_files_mode(v: Option<&String>) -> FilesMode {
    match v.map(|s| s.to_lowercase()) {
        Some(ref s) if s == "embed" => FilesMode::Embed,
        Some(ref s) if s == "ref" => FilesMode::Ref,
        _ => FilesMode::Auto,
    }
}

fn parse_files_encoding(v: Option<&String>) -> FilesEncoding {
    match v.map(|s| s.to_lowercase()) {
        Some(ref s) if s == "utf-8" || s == "utf8" => FilesEncoding::Utf8,
        Some(ref s) if s == "base64" => FilesEncoding::Base64,
        _ => FilesEncoding::Auto,
    }
}

fn validate_id(id: &str) -> Result<(), StdioError> {
    static RESERVED: &[&str] = &[
        "_root", "_start", "_end", "_all", "_none", "_self", "_parent",
    ];
    if RESERVED.contains(&id) || id.starts_with("__") {
        return Err(StdioError::InvalidId(id.to_string()));
    }
    let re = Regex::new(r"^[a-zA-Z_][a-zA-Z0-9_\-\.]{0,127}$").unwrap();
    if !re.is_match(id) {
        return Err(StdioError::InvalidId(id.to_string()));
    }
    Ok(())
}

fn validate_dependencies(tasks: &[StdioTask]) -> Result<(), StdioError> {
    let mut ids: HashSet<&str> = HashSet::new();
    for t in tasks {
        if !ids.insert(&t.id) {
            return Err(StdioError::DuplicateId(t.id.clone()));
        }
    }
    for t in tasks {
        for dep in &t.dependencies {
            if !ids.contains(dep.as_str()) {
                return Err(StdioError::UnknownDependency {
                    task: t.id.clone(),
                    dep: dep.clone(),
                });
            }
        }
    }

    let mut visiting = HashSet::new();
    let mut visited = HashSet::new();
    let lookup: HashMap<&str, &StdioTask> = tasks.iter().map(|t| (t.id.as_str(), t)).collect();

    fn dfs<'a>(
        id: &'a str,
        lookup: &HashMap<&'a str, &'a StdioTask>,
        visiting: &mut HashSet<&'a str>,
        visited: &mut HashSet<&'a str>,
    ) -> bool {
        if visited.contains(id) {
            return false;
        }
        if !visiting.insert(id) {
            return true;
        }
        if let Some(task) = lookup.get(id) {
            for dep in &task.dependencies {
                if dfs(dep, lookup, visiting, visited) {
                    return true;
                }
            }
        }
        visiting.remove(id);
        visited.insert(id);
        false
    }

    for id in ids {
        if dfs(id, &lookup, &mut visiting, &mut visited) {
            return Err(StdioError::CircularDependency);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_single_task_preserves_multiline_content() {
        let input = r#"
---TASK---
id: t1
backend: codex
workdir: .
---CONTENT---
line1
line2
---END---
"#;
        let tasks = parse_stdio_tasks(input).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].content, "line1\nline2");
        assert_eq!(tasks[0].stream_format, "text");
    }

    #[test]
    fn parse_generates_id_when_missing() {
        let input = r#"
---TASK---
backend: codex
workdir: .
---CONTENT---
hello
---END---
"#;
        let tasks = parse_stdio_tasks(input).unwrap();
        assert_eq!(tasks.len(), 1);
        assert!(!tasks[0].id.trim().is_empty());
        assert!(tasks[0].id.starts_with("task-"));
    }

    #[test]
    fn parse_validates_unknown_dependency() {
        let input = r#"
---TASK---
id: a
backend: codex
workdir: .
dependencies: b
---CONTENT---
hello
---END---
"#;
        let err = parse_stdio_tasks(input).unwrap_err();
        assert!(matches!(err, StdioError::UnknownDependency { .. }));
    }

    #[test]
    fn parse_detects_cycle() {
        let input = r#"
---TASK---
id: a
backend: codex
workdir: .
dependencies: b
---CONTENT---
a
---END---

---TASK---
id: b
backend: codex
workdir: .
dependencies: a
---CONTENT---
b
---END---
"#;
        let err = parse_stdio_tasks(input).unwrap_err();
        assert!(matches!(err, StdioError::CircularDependency));
    }
}
