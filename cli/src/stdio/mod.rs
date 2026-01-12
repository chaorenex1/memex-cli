pub mod execute;
pub mod input;

pub use execute::execute_stdio_tasks;
pub use input::{decode_stdin_bytes, read_stdin_text};
