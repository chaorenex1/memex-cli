use serde_json::Value;

use crate::tool_event::ToolEvent;

#[derive(Debug, Clone)]
pub struct ToolEventLite {
    pub tool: String,
    pub action: Option<String>,
    pub args: Value,
    pub ok: Option<bool>,
}

impl From<&ToolEvent> for ToolEventLite {
    fn from(ev: &ToolEvent) -> Self {
        Self {
            tool: ev.tool.clone().unwrap_or_default(),
            action: ev.action.clone(),
            args: ev.args.clone(),
            ok: ev.ok,
        }
    }
}
