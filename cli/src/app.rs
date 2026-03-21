//! CLI 应用装配层：合并配置覆盖，并分发到标准执行流。
use crate::commands::cli::{Args, RunArgs};
use memex_core::api as core_api;

use crate::flow::standard;

#[tracing::instrument(name = "cli.run_app", skip(args, run_args, ctx))]
pub async fn run_app_with_config(
    args: Args,
    run_args: Option<RunArgs>,
    recover_run_id: Option<String>,
    is_remote: &bool,
    ctx: &core_api::AppContext,
) -> Result<i32, core_api::RunnerError> {
    let args = args;
    standard::run_standard_flow(
        &args,
        run_args.as_ref(),
        ctx,
        is_remote,
        recover_run_id.clone(),
    )
    .await
}
