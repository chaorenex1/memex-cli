use clap::{Parser, Subcommand};

use crate::replay::cli::ReplayArgs;

#[derive(Parser, Debug)]
pub struct Args {
    #[command(subcommand)]
    pub command: Option<Commands>,

    #[arg(long, default_value = "codex")]
    pub codecli_bin: String,

    #[arg(trailing_var_arg = true)]
    pub codecli_args: Vec<String>,

    #[arg(long, default_value_t = 65536)]
    pub capture_bytes: usize,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    Replay(ReplayArgs),
}
