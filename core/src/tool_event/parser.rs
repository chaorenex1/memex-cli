pub use crate::tool_event::ToolEvent;

pub trait ToolEventParser: Send {
    fn parse_line(&mut self, line: &str) -> Option<ToolEvent>;
    fn format_line(&self, ev: &ToolEvent) -> String;
}

pub struct PrefixedJsonlParser {
    prefix: &'static str,
}

impl PrefixedJsonlParser {
    pub fn new(prefix: &'static str) -> Self {
        Self { prefix }
    }
}

impl ToolEventParser for PrefixedJsonlParser {
    fn parse_line(&mut self, line: &str) -> Option<ToolEvent> {
        let s = line.trim();
        if !s.starts_with(self.prefix) {
            return None;
        }
        let json_part = s[self.prefix.len()..].trim();
        if json_part.is_empty() {
            return None;
        }
        serde_json::from_str::<ToolEvent>(json_part).ok()
    }

    fn format_line(&self, ev: &ToolEvent) -> String {
        let json = serde_json::to_string(ev).unwrap_or_else(|_| "{}".to_string());
        format!("{} {}", self.prefix, json)
    }
}
