use clap::Parser;
mod app;
mod commands;
use commands::cli;
use memex_core::error;
use memex_core::replay;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let exit = match real_main().await {
        Ok(code) => code,
        Err(e) => {
            eprintln!("{e}");
            exit_code_for_error(&e)
        }
    };

    std::process::exit(exit);
}

async fn real_main() -> Result<i32, error::CliError> {
    let mut args = cli::Args::parse();
    let cmd = args.command.take();

    if let Some(cmd) = cmd {
        return dispatch(cmd, args).await;
    }

    let exit = app::run_app(args, None, None).await?;
    Ok(exit)
}

fn exit_code_for_error(e: &error::CliError) -> i32 {
    // 0: success
    // 11: config error
    // 20: runner start / IO error
    // 40: policy deny (usually returned as a normal exit code, not as an error)
    // 50: internal/uncategorized
    match e {
        error::CliError::Config(_) => 11,
        error::CliError::Runner(re) => match re {
            error::RunnerError::Config(_) => 11,
            error::RunnerError::Spawn(_) => 20,
            error::RunnerError::StreamIo { .. } => 20,
            error::RunnerError::Plugin(_) => 50,
        },
        error::CliError::Io(_) => 20,
        error::CliError::Command(_) => 20,
        error::CliError::Replay(_) => 50,
        error::CliError::Anyhow(_) => 50,
    }
}

async fn dispatch(cmd: cli::Commands, args: cli::Args) -> Result<i32, error::CliError> {
    match cmd {
        cli::Commands::Run(run_args) => {
            let exit = app::run_app(args, Some(run_args), None).await?;
            Ok(exit)
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
            Ok(0)
        }
        cli::Commands::Resume(resume_args) => {
            let recover_id = Some(resume_args.run_id.clone());
            let exit = app::run_app(args, Some(resume_args.run_args), recover_id).await?;
            Ok(exit)
        }
    }
}
