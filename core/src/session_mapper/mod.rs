//! Session Mapper - CLI run_id 与 backend session_id 的映射存储
//!
//! 存储路径: ~/.memex/sessions/<run_id>/state.json
//!
//! 功能:
//! - 一对多映射: 一个 CLI run_id 可对应多个 backend session_id
//! - 单任务支持 resume
//! - 多任务不支持 resume (报错)

mod types;
mod mapper;

pub use types::{BackendSession, ExecutionStatus, SessionState};
pub use mapper::{ResumeCheck, SessionMapper};
