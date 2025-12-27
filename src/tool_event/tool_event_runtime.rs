use crate::events_out::EventsOutTx;
use crate::tool_event::{ToolEvent, ToolEventParser};

pub struct ToolEventRuntime<P: ToolEventParser> {
    parser: P,
    events: Vec<ToolEvent>,
    events_out: Option<EventsOutTx>,
}

impl<P: ToolEventParser> ToolEventRuntime<P> {
    pub fn new(parser: P, events_out: Option<EventsOutTx>) -> Self {
        Self {
            parser,
            events: Vec::new(),
            events_out,
        }
    }

    pub async fn observe_line(&mut self, line: &str) {
        if let Some(ev) = self.parser.parse_line(line) {
            self.events.push(ev.clone());

            if let Some(out) = &self.events_out {
                let s = self.parser.format_line(&ev);
                out.send_line(s).await;
            }
        }
    }

    pub fn take_events(&mut self) -> Vec<ToolEvent> {
        std::mem::take(&mut self.events)
    }

    pub fn dropped_events_out(&self) -> u64 {
        self.events_out
            .as_ref()
            .map(|x| x.dropped_count())
            .unwrap_or(0)
    }
}

