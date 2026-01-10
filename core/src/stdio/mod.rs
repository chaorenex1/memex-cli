mod error;
mod executor;
mod id_gen;
pub mod metrics;
mod parser;
mod render;
mod retry;
mod types;

pub use error::{ErrorCode, StdioError, StdioParseError};
pub use executor::run_stdio;
pub use id_gen::generate_task_id;
pub use parser::parse_stdio_tasks;
pub use render::{
    configure_event_buffer, emit_json, flush_event_buffer, render_task_jsonl, render_task_stream,
    JsonlEvent, RenderOutcome, RenderTaskInfo, TextMarkers,
};
pub use retry::{effective_timeout_secs, max_attempts};
pub use types::{FilesEncoding, FilesMode, StdioRunOpts, StdioTask};
