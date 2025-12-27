use crate::config::model::Config;
// core/src/app.rs
use crate::runner::r#trait::{Runner, RunnerSpec, StreamSpec};
use crate::memory::r#trait::{MemoryClient, SearchRequest};
use crate::gatekeeper::r#trait::{Gatekeeper, GatekeeperInput};
use crate::policy::r#trait::{PolicyEngine, Approver};
use crate::types::TraceContext;
use std::sync::Arc;

pub struct AppContext {
    pub config: Config,
    pub runner: Arc<dyn Runner>,
    pub memory: Arc<dyn MemoryClient>,
    pub gatekeeper: Arc<dyn Gatekeeper>,
    pub policy: Arc<dyn PolicyEngine>,
    pub approver: Arc<dyn Approver>,
}

impl AppContext {
    pub fn new(
        config: Config,
        runner: Arc<dyn Runner>,
        memory: Arc<dyn MemoryClient>,
        gatekeeper: Arc<dyn Gatekeeper>,
        policy: Arc<dyn PolicyEngine>,
        approver: Arc<dyn Approver>,
    ) -> Self {
        Self {
            config,
            runner,
            memory,
            gatekeeper,
            policy,
            approver,
        }
    }
}

pub struct App {
    pub ctx: AppContext,
}

impl App {
    pub fn new(ctx: AppContext) -> Self {
        Self { ctx }
    }

    pub async fn run_pipeline(&self, trace: TraceContext, args: Vec<String>)
        -> anyhow::Result<i32> /* exit code */ 
    {
        // 1) build query
        let query = args.join(" ");

        // 2) memory.search
        let search_res = self.ctx.memory.search(SearchRequest {
            project_id: trace.project_id.clone(),
            query: query.clone(),
            limit: 5,
            min_score: 0.5,
        }).await?;

        // 3) build injected prompt/context (simplified)
        // In reality, this would be injected into codecli via env or prompt file
        
        // 4) runner.run
        let start_time = std::time::Instant::now();
        let output = self.ctx.runner.run(
            &trace,
            RunnerSpec {
                program: "codecli".to_string(),
                args: args.clone(), // Use actual args
                cwd: None,
                env: vec![],
            },
            StreamSpec {
                stream_stdout: true,
                stream_stderr: true,
                max_capture_bytes: 1024 * 1024,
            },
            None, None, None
        ).await?;
        let duration = start_time.elapsed();

        // 5) memory.hit (shown/used) - placeholder
        
        // 6) gatekeeper.evaluate
        let decision = self.ctx.gatekeeper.evaluate(GatekeeperInput {
            mode: crate::types::GatekeeperMode::Soft,
            redact_level: crate::types::RedactLevel::Basic,
            user_query: query,
            injected_items: search_res,
            final_stdout: String::from_utf8_lossy(&output.captured.stdout).to_string(),
            final_stderr: String::from_utf8_lossy(&output.captured.stderr).to_string(),
            exit_code: output.status_code,
            duration_ms: duration.as_millis() as u64,
        }).await?;

        // 7) memory.candidate / memory.validate
        if decision.should_validate {
            for req in decision.validate {
                // In a real app, this should be done asynchronously or in background
                if let Err(e) = self.ctx.memory.validate(req).await {
                    eprintln!("Failed to send validation: {}", e);
                }
            }
        }
        
        if decision.should_write_candidate {
            if let Some(req) = decision.candidate {
                if let Err(e) = self.ctx.memory.candidate(req).await {
                    eprintln!("Failed to write candidate: {}", e);
                }
            }
        }

        // 8) return codecli status
        Ok(output.status_code)
    }
}

