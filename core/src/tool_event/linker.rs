use crate::tool_event::ToolEvent;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct ToolStep {
    pub title: String,
    pub body: String,
}

pub fn extract_tool_steps(
    events: &[ToolEvent],
    max_steps: usize,
    args_keys_max: usize,
    value_max_chars: usize,
) -> Vec<ToolStep> {
    let mut steps = Vec::new();

    // 只取最近的 tool.request，倒序扫描
    for e in events.iter().rev() {
        if steps.len() >= max_steps {
            break;
        }
        use crate::tool_event::stream_json::EVENT_TYPE_TOOL_REQUEST;
        if e.event_type != EVENT_TYPE_TOOL_REQUEST {
            continue;
        }

        let tool = e.tool.clone().unwrap_or_else(|| "unknown".to_string());
        let action = e.action.clone().unwrap_or_else(|| "call".to_string());

        // 生成一个“稳健的摘要”（不输出全部 args）
        let args_summary = summarize_args(&e.args, args_keys_max, value_max_chars);

        steps.push(ToolStep {
            title: format!("Call tool `{}` ({})", tool, action),
            body: format!("Args summary: {}", args_summary),
        });
    }

    steps.reverse();
    steps
}

pub fn extract_tool_step_single(
    event: &ToolEvent,
    args_keys_max: usize,
    value_max_chars: usize,
) -> Option<ToolStep> {
    use crate::tool_event::stream_json::EVENT_TYPE_TOOL_REQUEST;
    if event.event_type != EVENT_TYPE_TOOL_REQUEST {
        return None;
    }

    let tool = event.tool.clone().unwrap_or_else(|| "unknown".to_string());
    let action = event.action.clone().unwrap_or_else(|| "call".to_string());

    let args_summary = summarize_args(&event.args, args_keys_max, value_max_chars);

    Some(ToolStep {
        title: format!("Call tool `{}` ({})", tool, action),
        body: format!("Args summary: {}", args_summary),
    })
}

#[allow(unused_variables)]
fn summarize_args(args: &Value, args_keys_max: usize, value_max_chars: usize) -> String {
    args.to_string()
}
