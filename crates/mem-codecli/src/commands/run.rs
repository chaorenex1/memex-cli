use memex_core::AppContext;
use memex_core::app::App;
use memex_core::types::{TraceContext, RunId, ProjectId};
use std::collections::BTreeMap;

pub fn handle(ctx: &AppContext) -> Result<(), String> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;

    let app = App::new(AppContext {
        config: ctx.config.clone(),
        runner: ctx.runner.clone(),
        memory: ctx.memory.clone(),
        gatekeeper: ctx.gatekeeper.clone(),
        policy: ctx.policy.clone(),
        approver: ctx.approver.clone(),
    });

    let trace = TraceContext {
        run_id: RunId(uuid::Uuid::new_v4().to_string()),
        project_id: ProjectId("default".to_string()),
        extra: BTreeMap::new(),
    };

    let args: Vec<String> = std::env::args().skip(2).collect();

    rt.block_on(async {
        match app.run_pipeline(trace, args).await {
            Ok(code) => {
                if code != 0 {
                    std::process::exit(code);
                }
                Ok(())
            }
            Err(e) => Err(e.to_string()),
        }
    })
}
