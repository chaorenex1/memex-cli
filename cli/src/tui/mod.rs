//! TUI 模块：终端输入读取、应用状态（TuiApp）与渲染（ui）。
pub(crate) mod app;
pub(crate) mod events;
mod terminal;
pub(crate) mod ui;

pub use app::{InputMode, PromptAction, RunStatus, TuiApp};
pub use terminal::{check_tui_support, restore_terminal, setup_terminal};
