use crate::tool_event::model::{ToolEvent, TOOL_EVENT_PREFIX};

pub fn parse_tool_event_line(line: &str) -> Option<ToolEvent> {
    let s = line.trim();
    if !s.starts_with(TOOL_EVENT_PREFIX) {
        return None;
    }
    let json_part = s[TOOL_EVENT_PREFIX.len()..].trim();
    if json_part.is_empty() {
        return None;
    }
    serde_json::from_str::<ToolEvent>(json_part).ok()
}

pub fn format_tool_event_line(ev: &ToolEvent) -> String {
    let json = serde_json::to_string(ev).unwrap_or_else(|_| "{}".to_string());
    format!("{TOOL_EVENT_PREFIX} {json}")
}
