use crate::tool_event::model::ToolEvent;
use crate::tool_event::{PrefixedJsonlParser, StreamJsonToolEventParser, ToolEventParser};

/// Stateful, best-effort parser for mixed stdout/stderr logs.
///
/// Supported inputs (in this order):
/// 1) Prefixed JSONL: `@@MEM_TOOL_EVENT@@ { ...ToolEvent... }`
/// 2) External stream-json formats (gemini/codex/claude) via `StreamJsonToolEventParser`
/// 3) Raw ToolEvent JSON (must match `ToolEvent` schema)
///
/// Use this when you need to parse a whole log sequentially (e.g. replay), where
/// some formats require cross-line correlation (like gemini tool_result without tool_name).
pub struct MultiToolEventLineParser {
    prefixed: PrefixedJsonlParser,
    stream_json: StreamJsonToolEventParser,
}

impl MultiToolEventLineParser {
    pub fn new(prefix: &'static str) -> Self {
        Self {
            prefixed: PrefixedJsonlParser::new(prefix),
            stream_json: StreamJsonToolEventParser::new(),
        }
    }

    pub fn parse_line(&mut self, line: &str) -> Option<ToolEvent> {
        self.prefixed
            .parse_line(line)
            .or_else(|| self.stream_json.parse_line(line))
            .or_else(|| serde_json::from_str::<ToolEvent>(line.trim()).ok())
    }
}
