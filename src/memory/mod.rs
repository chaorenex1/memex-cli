pub mod adapters;
pub mod client;
pub mod models;

use chrono::Utc;
use regex::Regex;
use serde_json::Value;

use crate::gatekeeper::{GatekeeperDecision, InjectItem};
use crate::tool_event::{extract_tool_steps, ToolEvent, ToolStep};
use crate::tool_event::ToolEventLite;

pub use adapters::parse_search_matches;
pub use client::MemoryClient;
pub use models::{
    QACandidatePayload, QAHitsPayload, QAReferencePayload, QASearchPayload, QAValidationPayload,
};

#[derive(Debug, Clone)]
pub struct CandidateDraft {
    pub question: String,
    pub answer: String,
    pub tags: Vec<String>,
    pub confidence: f32,
    pub metadata: Value,
    pub summary: Option<String>,
    pub source: Option<String>,
}

pub fn build_hit_payload(project_id: &str, decision: &GatekeeperDecision) -> Option<QAHitsPayload> {
    if decision.hit_refs.is_empty() {
        return None;
    }

    let refs = decision
        .hit_refs
        .iter()
        .map(|r| QAReferencePayload {
            qa_id: r.qa_id.clone(),
            shown: Some(r.shown),
            used: Some(r.used),
            message_id: r.message_id.clone(),
            context: r.context.clone(),
        })
        .collect::<Vec<_>>();

    Some(QAHitsPayload {
        project_id: project_id.to_string(),
        references: refs,
    })
}

pub fn build_validate_payloads(
    project_id: &str,
    decision: &GatekeeperDecision,
) -> Vec<QAValidationPayload> {
    decision
        .validate_plans
        .iter()
        .map(|p| QAValidationPayload {
            project_id: project_id.to_string(),
            qa_id: p.qa_id.clone(),
            result: Some(p.result.clone()),
            signal_strength: Some(p.signal_strength.clone()),
            strong_signal: Some(p.strong_signal),
            context: p.context.clone(),
            ts: Some(Utc::now().to_rfc3339()),
            payload: Some(p.payload.clone()),
            source: Some("mem-codecli".to_string()),
            client: None,
            success: None,
        })
        .collect()
}

pub fn build_candidate_payloads(
    project_id: &str,
    drafts: &[CandidateDraft],
) -> Vec<QACandidatePayload> {
    drafts
        .iter()
        .map(|d| QACandidatePayload {
            project_id: project_id.to_string(),
            question: d.question.clone(),
            answer: d.answer.clone(),
            tags: d.tags.clone(),
            confidence: d.confidence,
            metadata: d.metadata.clone(),
            summary: d.summary.clone(),
            source: d.source.clone(),
            author: None,
        })
        .collect()
}

#[derive(Debug, Clone, Copy)]
pub enum InjectPlacement {
    System,
    User,
}

#[derive(Debug, Clone)]
pub struct InjectConfig {
    pub placement: InjectPlacement,
    pub max_items: usize,
    pub max_answer_chars: usize,
    pub include_meta_line: bool,
}

impl Default for InjectConfig {
    fn default() -> Self {
        Self {
            placement: InjectPlacement::System,
            max_items: 3,
            max_answer_chars: 900,
            include_meta_line: true,
        }
    }
}

pub fn render_memory_context(items: &[InjectItem], cfg: &InjectConfig) -> String {
    if items.is_empty() {
        return String::new();
    }

    let mut out = String::new();
    out.push_str("[MEMORY_CONTEXT v1]\n");
    out.push_str("The following items are retrieved from the memory system. Prefer using them when relevant.\n");
    out.push_str("If you use an item, include its anchor exactly once in your final answer: [QA_REF <qa_id>].\n\n");

    for (idx, it) in items.iter().take(cfg.max_items).enumerate() {
        let n = idx + 1;
        out.push_str(&format!("{n}) [QA_REF {}]\n", it.qa_id));
        out.push_str(&format!("Q: {}\n", one_line(&it.question)));
        let a = pick_answer(it, cfg.max_answer_chars);
        out.push_str(&format!("A: {}\n", a));

        if cfg.include_meta_line {
            out.push_str(&format!(
                "Meta: level={} trust={:.2} score={:.2} tags={}\n",
                it.validation_level,
                it.trust,
                it.score,
                if it.tags.is_empty() { "-".to_string() } else { it.tags.join(",") }
            ));
        }
        out.push('\n');
    }

    out.push_str("Rules:\n");
    out.push_str("- Do not invent anchors.\n");
    out.push_str("- If none are relevant, ignore them.\n");
    out.push_str("- Prefer the highest validation_level and trust.\n");
    out.push_str("[/MEMORY_CONTEXT]\n");

    out
}

pub fn merge_prompt(user_query: &str, memory_context: &str) -> String {
    if memory_context.trim().is_empty() {
        return user_query.to_string();
    }
    format!("{memory_context}\n{user_query}")
}

fn pick_answer(it: &InjectItem, max_chars: usize) -> String {
    let raw = if let Some(s) = &it.summary {
        s.as_str()
    } else {
        it.answer.as_str()
    };
    truncate_clean(raw, max_chars)
}

fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate_clean(s: &str, max_chars: usize) -> String {
    let mut t = s.trim().to_string();
    t = t.replace("\r\n", "\n");
    if t.chars().count() <= max_chars {
        return t;
    }
    let mut out = String::new();
    for (i, ch) in t.chars().enumerate() {
        if i >= max_chars {
            break;
        }
        out.push(ch);
    }
    out.push_str(" ...");
    out
}

#[derive(Debug, Clone)]
pub struct CandidateExtractConfig {
    pub max_candidates: usize,
    pub max_answer_chars: usize,
    pub min_answer_chars: usize,
    pub context_lines: usize,
    pub redact: bool,
    pub strict_secret_block: bool,
}

impl Default for CandidateExtractConfig {
    fn default() -> Self {
        Self {
            max_candidates: 1,
            max_answer_chars: 1200,
            min_answer_chars: 200,
            context_lines: 8,
            redact: true,
            strict_secret_block: true,
        }
    }
}

pub fn extract_candidates(
    cfg: &CandidateExtractConfig,
    user_query: &str,
    stdout_tail: &str,
    stderr_tail: &str,
    tool_events: &[ToolEventLite],
) -> Vec<CandidateDraft> {
    if cfg.max_candidates == 0 {
        return vec![];
    }

    let mut combined = String::new();
    if !stdout_tail.trim().is_empty() {
        combined.push_str(stdout_tail);
        combined.push('\n');
    }
    if !stderr_tail.trim().is_empty() {
        combined.push_str(stderr_tail);
        combined.push('\n');
    }

    if cfg.strict_secret_block && contains_secret(&combined) {
        return vec![];
    }

    let cmd_block = extract_command_block(stdout_tail, cfg.context_lines)
        .or_else(|| extract_command_block(stderr_tail, cfg.context_lines));

    let err_hint = extract_error_hint(stderr_tail).or_else(|| extract_error_hint(stdout_tail));

    let tool_summary = summarize_tool_events(tool_events);

    let question = build_question(user_query, err_hint.as_deref(), tool_events);

    let mut answer = String::new();

    answer.push_str("## Context\n");
    answer.push_str(&format!("- Task: {}\n", one_line(user_query)));
    if let Some(h) = &err_hint {
        answer.push_str(&format!("- Error hint: {}\n", one_line(h)));
    }
    if !tool_summary.trim().is_empty() {
        answer.push_str(&format!("- Tools observed: {}\n", tool_summary));
    }
    answer.push('\n');

    let tool_steps = extract_tool_steps_from_lite(tool_events, 5);

    answer.push_str("## Steps\n");
    if !tool_steps.is_empty() {
        for (i, s) in tool_steps.iter().enumerate() {
            answer.push_str(&format!("{}. {}\n", i + 1, s.title));
            answer.push_str(&format!("   - {}\n", s.body));
        }
    } else if let Some(ref block) = cmd_block {
        answer.push_str("1. Run the following commands:\n```bash\n");
        answer.push_str(block);
        if !block.ends_with('\n') {
            answer.push('\n');
        }
        answer.push_str("```\n");
    } else {
        answer.push_str("1. Identify the failing command/output in your terminal logs.\n");
        answer.push_str("2. Apply the fix corresponding to the error hint.\n");
        answer.push_str("3. Re-run tests/build to confirm.\n");
    }

    answer.push_str("\n## Notes\n");
    if let Some(h) = &err_hint {
        answer.push_str(&format!(
            "- If you see `{}`, focus on the dependency/configuration causing it.\n",
            trim_mid(h, 80)
        ));
    } else {
        answer.push_str("- If the fix doesn't work, capture the exact error line and tool versions.\n");
    }
    answer.push_str("- Keep secrets (tokens/keys/passwords) out of logs and configs.\n");

    let mut final_answer = answer;
    if cfg.redact {
        final_answer = redact_secrets(&final_answer);
    }

    if final_answer.chars().count() < cfg.min_answer_chars {
        return vec![];
    }

    final_answer = truncate_clean(&final_answer, cfg.max_answer_chars);

    let tags = infer_tags(user_query, &final_answer, tool_events);

    let draft = CandidateDraft {
        question,
        answer: final_answer,
        tags,
        confidence: 0.45,
        metadata: serde_json::json!({
            "source": "heuristic_extractor_v1",
            "has_cmd_block": cmd_block.is_some(),
            "has_error_hint": err_hint.is_some(),
        }),
        summary: None,
        source: Some("mem-codecli".to_string()),
    };

    vec![draft]
}

fn extract_tool_steps_from_lite(events: &[ToolEventLite], max: usize) -> Vec<ToolStep> {
    let real_events: Vec<ToolEvent> = events
        .iter()
        .map(|lite| ToolEvent {
            v: 1,
            event_type: "tool.request".to_string(),
            ts: None,
            id: None,
            tool: Some(lite.tool.clone()),
            action: lite.action.clone(),
            args: lite.args.clone(),
            ok: lite.ok,
            output: None,
        })
        .collect();

    extract_tool_steps(&real_events, max)
}

fn extract_command_block(text: &str, context_lines: usize) -> Option<String> {
    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return None;
    }

    let cmd_re = Regex::new(
        r#"^(?:\s*\$\s+|\s*(cargo|git|npm|pnpm|yarn|bun|go|pytest|python|pip|uv|uvx|docker|kubectl)\b)"#,
    )
    .ok()?;

    let mut last_idx: Option<usize> = None;
    for (i, l) in lines.iter().enumerate() {
        if cmd_re.is_match(l) {
            last_idx = Some(i);
        }
    }
    let idx = last_idx?;

    let start = idx.saturating_sub(context_lines);
    let end = (idx + context_lines + 1).min(lines.len());

    let mut out = String::new();
    for l in &lines[start..end] {
        let s = l.trim_end();
        if s.is_empty() {
            continue;
        }
        out.push_str(s);
        out.push('\n');
    }

    if out.trim().is_empty() { None } else { Some(out) }
}

fn extract_error_hint(text: &str) -> Option<String> {
    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return None;
    }

    let err_re = Regex::new(r#"(?i)\b(error|failed|panic|exception|traceback)\b"#).ok()?;

    for l in lines.iter().rev() {
        let s = l.trim();
        if s.len() < 6 {
            continue;
        }
        if err_re.is_match(s) {
            return Some(s.to_string());
        }
    }
    None
}

fn summarize_tool_events(events: &[ToolEventLite]) -> String {
    if events.is_empty() {
        return String::new();
    }
    let mut names: Vec<String> = Vec::new();
    for e in events.iter().rev().take(3) {
        let mut t = e.tool.clone();
        if let Some(a) = &e.action {
            t = format!("{}:{}", t, a);
        }
        names.push(t);
    }
    names.reverse();
    names.join(", ")
}

fn build_question(user_query: &str, err_hint: Option<&str>, tool_events: &[ToolEventLite]) -> String {
    if let Some(h) = err_hint {
        return format!(
            "How to resolve `{}` when running: {}",
            trim_mid(h, 90),
            trim_mid(user_query, 120)
        );
    }

    if let Some(t) = tool_events.last() {
        return format!(
            "How to complete task using tool `{}` for: {}",
            t.tool,
            trim_mid(user_query, 140)
        );
    }

    format!("How to: {}", trim_mid(user_query, 180))
}

fn infer_tags(user_query: &str, answer: &str, tool_events: &[ToolEventLite]) -> Vec<String> {
    let mut tags = Vec::new();
    let s = format!("{}\n{}", user_query, answer).to_lowercase();

    if s.contains("cargo") || s.contains("rust") {
        tags.push("rust".into());
    }
    if s.contains("npm") || s.contains("pnpm") || s.contains("node") {
        tags.push("nodejs".into());
    }
    if s.contains("pytest") || s.contains("python") || s.contains("pip") || s.contains("uv") {
        tags.push("python".into());
    }
    if s.contains("docker") {
        tags.push("docker".into());
    }
    if s.contains("kubernetes") || s.contains("kubectl") {
        tags.push("k8s".into());
    }
    if s.contains("mcp") {
        tags.push("mcp".into());
    }

    for e in tool_events.iter() {
        let t = e.tool.to_lowercase();
        if t.contains("git") && !tags.contains(&"git".to_string()) {
            tags.push("git".into());
        }
        if t.contains("fs") && !tags.contains(&"filesystem".to_string()) {
            tags.push("filesystem".into());
        }
    }

    tags.sort();
    tags.dedup();
    tags
}

fn contains_secret(s: &str) -> bool {
    let patterns = secret_patterns();
    patterns.iter().any(|re| re.is_match(s))
}

fn redact_secrets(s: &str) -> String {
    let mut out = s.to_string();
    for re in secret_patterns() {
        out = re.replace_all(&out, "[REDACTED]").to_string();
    }
    out
}

fn secret_patterns() -> Vec<Regex> {
    vec![
        Regex::new(r"(?i)\b(sk-[A-Za-z0-9]{20,})\b").unwrap(),
        Regex::new(r"\bAKIA[0-9A-Z]{16}\b").unwrap(),
        Regex::new(r"(?i)\b(ghp|gho|ghu|ghs|ghr)_[A-Za-z0-9]{20,}\b").unwrap(),
        Regex::new(r"\beyJ[A-Za-z0-9_\-]+=*\.[A-Za-z0-9_\-]+=*\.[A-Za-z0-9_\-]+=*\b").unwrap(),
        Regex::new(r"-----BEGIN (RSA|EC|OPENSSH|DSA)? ?PRIVATE KEY-----").unwrap(),
        Regex::new(r"(?i)\b[a-z]+:\/\/[^\/\s:]+:[^\/\s@]+@").unwrap(),
    ]
}

fn trim_mid(s: &str, max_chars: usize) -> String {
    let t = one_line(s);
    if t.chars().count() <= max_chars {
        return t;
    }
    let head: String = t.chars().take(max_chars.saturating_sub(2)).collect();
    format!("{head}..")
}

