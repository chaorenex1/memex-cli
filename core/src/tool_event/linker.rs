use crate::tool_event::ToolEvent;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct ToolStep {
    pub title: String,
    pub body: String,
}

pub fn extract_tool_steps(events: &[ToolEvent], max_steps: usize) -> Vec<ToolStep> {
    let mut steps = Vec::new();

    // 只取最近的 tool.request，倒序扫描
    for e in events.iter().rev() {
        if steps.len() >= max_steps {
            break;
        }
        if e.event_type != "tool.request" {
            continue;
        }

        let tool = e.tool.clone().unwrap_or_else(|| "unknown".to_string());
        let action = e.action.clone().unwrap_or_else(|| "call".to_string());

        // 生成一个“稳健的摘要”（不输出全部 args）
        let args_summary = summarize_args(&e.args);

        steps.push(ToolStep {
            title: format!("Call tool `{}` ({})", tool, action),
            body: format!("Args summary: {}", args_summary),
        });
    }

    steps.reverse();
    steps
}

fn summarize_args(args: &Value) -> String {
    // 优先：如果有常见字段（query/path/url/code）就提取；否则列 keys
    if let Some(o) = args.as_object() {
        for k in ["query", "q", "path", "filepath", "file", "url", "command", "cmd", "code"].iter() {
            if let Some(v) = o.get(*k) {
                return format!("{}={}", k, shorten(v));
            }
        }
        let keys: Vec<String> = o.keys().cloned().take(16).collect();
        return format!("keys=[{}]", keys.join(","));
    }
    "non-object args".to_string()
}

fn shorten(v: &Value) -> String {
    let s = match v {
        Value::String(x) => x.clone(),
        _ => v.to_string(),
    };
    let t = s.trim().replace('\n', " ");
    if t.chars().count() <= 140 {
        t
    } else {
        t.chars().take(138).collect::<String>() + "…"
    }
}
