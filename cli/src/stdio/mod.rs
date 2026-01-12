pub mod input;
pub mod execute;

pub use input::{decode_stdin_bytes, read_stdin_text};
pub use execute::execute_stdio_tasks;
