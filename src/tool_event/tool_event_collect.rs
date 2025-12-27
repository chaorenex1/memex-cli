use crate::tool_event::{parse_tool_event_line, ToolEvent};

#[derive(Default)]
pub struct ToolEventCollector {
    pub events: Vec<ToolEvent>,
}

impl ToolEventCollector {
    pub fn observe_line(&mut self, line: &str) {
        if let Some(ev) = parse_tool_event_line(line) {
            self.events.push(ev);
        }
    }
}
