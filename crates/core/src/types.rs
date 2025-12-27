// core/src/types.rs
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct ProjectId(pub String);

#[derive(Clone, Debug)]
pub struct RunId(pub String);

#[derive(Clone, Debug)]
pub struct ToolName(pub String);

#[derive(Clone, Debug)]
pub enum InjectMode { System, User, Both, None }

#[derive(Clone, Debug)]
pub enum GatekeeperMode { Off, Soft, Hard }

#[derive(Clone, Debug)]
pub enum AuditMode { Off, Prompt, Auto }

#[derive(Clone, Debug)]
pub enum RedactLevel { Off, Basic, Strict }

#[derive(Clone, Debug)]
pub enum HitMode { Off, Shown, Used, Both }

#[derive(Clone, Debug)]
pub enum CandidateMode { Off, Auto, Force }

#[derive(Clone, Debug)]
pub enum ValidateMode { Off, Auto }

#[derive(Clone, Debug)]
pub struct TraceContext {
    pub run_id: RunId,
    pub project_id: ProjectId,
    pub extra: BTreeMap<String, String>,
}

