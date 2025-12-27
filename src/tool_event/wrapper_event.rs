use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WrapperEvent {
    pub v: i32,
    #[serde(rename = "type")]
    pub event_type: String,
    pub ts: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl WrapperEvent {
    pub fn new(event_type: &str, ts: String) -> Self {
        Self {
            v: 1,
            event_type: event_type.to_string(),
            ts,
            run_id: None,
            data: None,
        }
    }
}
