#[derive(Debug, Clone)]
pub struct ReplayArgs {
    pub events: String,
    pub run_id: Option<String>,
    pub format: String,
    pub set: Vec<String>,
    pub rerun_gatekeeper: bool,
}
