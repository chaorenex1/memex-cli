use clap::Args;

#[derive(Debug, Clone, Args)]
pub struct ReplayArgs {
    #[arg(long)]
    pub events: String,

    #[arg(long)]
    pub run_id: Option<String>,

    #[arg(long, default_value = "text")]
    pub format: String,

    #[arg(long, action = clap::ArgAction::Append)]
    pub set: Vec<String>,

    #[arg(long, default_value_t = false)]
    pub rerun_gatekeeper: bool,
}
