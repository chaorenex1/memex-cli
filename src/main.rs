mod cli;
mod config;
mod error;
mod events_out;
mod gatekeeper;
mod memory;
mod replay;
mod runner;
mod tool_event;
mod util;

use clap::Parser;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), error::CliError> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let args = cli::Args::parse();

    if let Some(cmd) = args.command {
        match cmd {
            cli::Commands::Replay(replay_args) => {
                let runs = replay::parse_events_file(&replay_args.events, replay_args.run_id.as_deref())
                    .map_err(error::CliError::Replay)?;
                let report = replay::report::build_report(&runs);

                if replay_args.format == "json" {
                    let s = serde_json::to_string_pretty(&report)
                        .map_err(|e| error::CliError::Replay(e.to_string()))?;
                    println!("{s}");
                } else {
                    let s = replay::report::format_text(&report);
                    println!("{s}");
                }
                return Ok(());
            }
        }
    }

    let exit = runner::run(args).await?;
    std::process::exit(exit);
}
