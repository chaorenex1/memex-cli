use clap::Parser;
mod app;
mod commands;
use commands::cli;
use memex_core::error;
use memex_core::replay;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), error::CliError> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let mut args = cli::Args::parse();
    let cmd = args.command.take();

    if let Some(cmd) = cmd {
        dispatch(cmd, args).await?;
        return Ok(());
    }

    let exit = app::run_app(args, None, None).await?;
    std::process::exit(exit);
}

async fn dispatch(cmd: cli::Commands, args: cli::Args) -> Result<(), error::CliError> {
    match cmd {
        cli::Commands::Run(run_args) => {
            let exit = app::run_app(args, Some(run_args), None).await?;
            std::process::exit(exit);
        }
        cli::Commands::Replay(replay_args) => {
            let core_args = replay::ReplayArgs {
                events: replay_args.events,
                run_id: replay_args.run_id,
                format: replay_args.format,
                set: replay_args.set,
                rerun_gatekeeper: replay_args.rerun_gatekeeper,
            };
            replay::replay_cmd(core_args).map_err(error::CliError::Replay)?;
        }
        cli::Commands::Resume(resume_args) => {
            let recover_id = Some(resume_args.run_id.clone());
            let exit = app::run_app(args, Some(resume_args.run_args), recover_id).await?;
            std::process::exit(exit);
        }
    }
    Ok(())
}
