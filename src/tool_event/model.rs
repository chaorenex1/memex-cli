use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const TOOL_EVENT_PREFIX: &str = "@@MEM_TOOL_EVENT@@";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolEvent {
    #[serde(default)]
    pub v: i32,

    #[serde(rename = "type")]
    pub event_type: String,

    #[serde(default)]
    pub ts: Option<String>,

    #[serde(default)]
    pub id: Option<String>,

    #[serde(default)]
    pub tool: Option<String>,

    #[serde(default)]
    pub action: Option<String>,

    #[serde(default)]
    pub args: Value,

    #[serde(default)]
    pub ok: Option<bool>,

    #[serde(default)]
    pub output: Option<Value>,
}
