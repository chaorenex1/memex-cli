use std::future::Future;

use chrono::Utc;

use crate::backend::BackendPlan;
use crate::error::RunnerError;
use crate::events_out::write_wrapper_event;
use crate::memory::{CandidateExtractConfig, InjectConfig, InjectPlacement};
use crate::runner::{RunnerResult, RunnerStartArgs};
use crate::tool_event::WrapperEvent;

use super::post::{post_run, PostRunContext};
use super::pre::{pre_run, EngineContext};
use super::types::{RunSessionInput, RunWithQueryArgs, RunnerSpec};

pub async fn run_with_query<F, Fut>(
    args: RunWithQueryArgs,
    run_session_fn: F,
) -> Result<i32, RunnerError>
where
    F: FnOnce(RunSessionInput) -> Fut,
    Fut: Future<Output = Result<RunnerResult, RunnerError>>,
{
    let RunWithQueryArgs {
        user_query,
        cfg,
        runner,
        run_id,
        capture_bytes,
        silent,
        events_out_tx,
        policy,
        memory,
        gatekeeper,
        wrapper_start_data,
    } = args;

    let inject_cfg: InjectConfig = InjectConfig {
        placement: match cfg.prompt_inject.placement {
            crate::config::PromptInjectPlacement::System => InjectPlacement::System,
            crate::config::PromptInjectPlacement::User => InjectPlacement::User,
        },
        max_items: cfg.prompt_inject.max_items,
        max_answer_chars: cfg.prompt_inject.max_answer_chars,
        include_meta_line: cfg.prompt_inject.include_meta_line,
    };

    let cand_cfg: CandidateExtractConfig = CandidateExtractConfig {
        max_candidates: cfg.candidate_extract.max_candidates,
        max_answer_chars: cfg.candidate_extract.max_answer_chars,
        min_answer_chars: cfg.candidate_extract.min_answer_chars,
        context_lines: cfg.candidate_extract.context_lines,
        tool_steps_max: cfg.candidate_extract.tool_steps_max,
        tool_step_args_keys_max: cfg.candidate_extract.tool_step_args_keys_max,
        tool_step_value_max_chars: cfg.candidate_extract.tool_step_value_max_chars,
        redact: cfg.candidate_extract.redact,
        strict_secret_block: cfg.candidate_extract.strict_secret_block,
        confidence: cfg.candidate_extract.confidence,
    };

    let (memory_search_limit, memory_min_score) = match &cfg.memory.provider {
        crate::config::MemoryProvider::Service(svc_cfg) => {
            (svc_cfg.search_limit, svc_cfg.min_score)
        }
    };

    let pre_ctx = EngineContext {
        project_id: &cfg.project_id,
        inject_cfg: &inject_cfg,
        memory: memory.as_deref(),
        gatekeeper: gatekeeper.as_ref(),
        memory_search_limit,
        memory_min_score,
    };

    let pre = pre_run(&pre_ctx, &user_query).await;
    let merged_query = pre.merged_query;
    let shown_qa_ids = pre.shown_qa_ids;
    let matches = pre.matches;
    let memory_search_event = pre.memory_search_event;

    // Buffer early wrapper events until we learn the effective run_id (session_id).
    // This keeps IDs consistent across the whole wrapper-event stream.
    let mut pending_wrapper_events: Vec<WrapperEvent> = Vec::new();
    if let Some(ev) = memory_search_event {
        pending_wrapper_events.push(ev);
    }

    let mut start_event = WrapperEvent::new("run.start", Utc::now().to_rfc3339());
    start_event.data = wrapper_start_data;
    pending_wrapper_events.push(start_event);

    // Build runner + session args (backend plan runs after memory injection)
    let (runner, session_args) = build_runner_and_args(runner, merged_query)?;

    // Always include the actual backend invocation in wrapper events for replay/observability.
    if let Some(last) = pending_wrapper_events.last_mut() {
        match last.data.as_mut() {
            Some(serde_json::Value::Object(map)) => {
                map.entry("cmd".to_string())
                    .or_insert_with(|| serde_json::Value::String(session_args.cmd.clone()));
                map.entry("args".to_string())
                    .or_insert_with(|| serde_json::json!(session_args.args.clone()));
            }
            None => {
                last.data = Some(serde_json::json!({
                    "cmd": session_args.cmd.clone(),
                    "args": session_args.args.clone(),
                }));
            }
            Some(_) => {
                last.data = Some(serde_json::json!({
                    "cmd": session_args.cmd.clone(),
                    "args": session_args.args.clone(),
                }));
            }
        }
    }

    // Start Session
    let session = match runner.start_session(&session_args).await {
        Ok(session) => session,
        Err(e) => {
            return Err(RunnerError::Spawn(e.to_string()));
        }
    };

    let run_input = RunSessionInput {
        session,
        run_id: run_id.clone(),
        control: cfg.control.clone(),
        policy,
        capture_bytes,
        events_out_tx: events_out_tx.clone(),
        silent,
    };

    // Run Session (runner runtime is in core; caller may provide a custom session loop, e.g. TUI).
    let run_result = match run_session_fn(run_input).await {
        Ok(r) => r,
        Err(e) => {
            // Best-effort: still emit buffered wrapper events so the run has a trace,
            // using the configured run_id (no session_id discovered).
            for mut ev in pending_wrapper_events {
                ev.run_id = Some(run_id.clone());
                write_wrapper_event(events_out_tx.as_ref(), &ev).await;
            }
            return Err(e);
        }
    };

    let effective_run_id = run_result.run_id.clone();

    // Flush buffered wrapper events with a consistent run_id.
    for mut ev in pending_wrapper_events {
        ev.run_id = Some(effective_run_id.clone());
        write_wrapper_event(events_out_tx.as_ref(), &ev).await;
    }

    if run_result.dropped_lines > 0 {
        let mut ev = WrapperEvent::new("tee.drop", Utc::now().to_rfc3339());
        ev.run_id = Some(effective_run_id.clone());
        ev.data = Some(serde_json::json!({ "dropped_lines": run_result.dropped_lines }));
        write_wrapper_event(events_out_tx.as_ref(), &ev).await;
    }

    let post_ctx = PostRunContext {
        project_id: &cfg.project_id,
        cand_cfg: &cand_cfg,
        memory: memory.as_deref(),
        gatekeeper: gatekeeper.as_ref(),
        events_out: events_out_tx.as_ref(),
    };

    let (run_outcome, _decision) =
        post_run(&post_ctx, &run_result, &matches, shown_qa_ids, &user_query).await?;

    let mut exit_event = WrapperEvent::new("run.end", Utc::now().to_rfc3339());
    exit_event.run_id = Some(effective_run_id);
    exit_event.data = Some(serde_json::json!({
        "exit_code": run_outcome.exit_code,
        "duration_ms": run_outcome.duration_ms,
        "stdout_tail": run_outcome.stdout_tail,
        "stderr_tail": run_outcome.stderr_tail,
        "used_qa_ids": run_outcome.used_qa_ids,
        "shown_qa_ids": run_outcome.shown_qa_ids,
    }));
    write_wrapper_event(events_out_tx.as_ref(), &exit_event).await;

    Ok(run_outcome.exit_code)
}

fn build_runner_and_args(
    runner: RunnerSpec,
    merged_query: String,
) -> Result<(Box<dyn crate::runner::RunnerPlugin>, RunnerStartArgs), RunnerError> {
    match runner {
        RunnerSpec::Backend {
            strategy,
            backend_spec,
            base_envs,
            resume_id,
            model,
            stream,
            stream_format,
        } => {
            let BackendPlan {
                runner,
                session_args,
            } = strategy
                .plan(
                    &backend_spec,
                    base_envs,
                    resume_id,
                    merged_query,
                    model,
                    stream,
                    &stream_format,
                )
                .map_err(|e| RunnerError::Spawn(e.to_string()))?;
            Ok((runner, session_args))
        }
        RunnerSpec::Passthrough {
            runner,
            session_args,
        } => Ok((runner, session_args)),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use serde_json::Value;

    use crate::backend::{BackendPlan, BackendStrategy};
    use crate::config::AppConfig;
    use crate::gatekeeper::{GatekeeperDecision, GatekeeperPlugin, InjectItem, SearchMatch};
    use crate::memory::MemoryPlugin;
    use crate::runner::{
        RunOutcome, RunnerPlugin, RunnerResult, RunnerSession, RunnerStartArgs, Signal,
    };

    use super::*;

    #[derive(Clone)]
    struct CaptureBackendStrategy {
        last_prompt: Arc<Mutex<Option<String>>>,
        runner: Arc<dyn RunnerPlugin>,
    }

    impl CaptureBackendStrategy {
        fn new(last_prompt: Arc<Mutex<Option<String>>>, runner: Arc<dyn RunnerPlugin>) -> Self {
            Self {
                last_prompt,
                runner,
            }
        }
    }

    impl BackendStrategy for CaptureBackendStrategy {
        fn name(&self) -> &str {
            "capture"
        }

        fn plan(
            &self,
            backend: &str,
            base_envs: HashMap<String, String>,
            _resume_id: Option<String>,
            prompt: String,
            _model: Option<String>,
            _stream: bool,
            _stream_format: &str,
        ) -> anyhow::Result<BackendPlan> {
            assert_eq!(backend, "dummy");
            assert!(base_envs.contains_key("PATH"));
            *self.last_prompt.lock().unwrap() = Some(prompt);
            Ok(BackendPlan {
                runner: Box::new(SharedRunnerPlugin(self.runner.clone())),
                session_args: RunnerStartArgs {
                    cmd: "dummy-cmd".to_string(),
                    args: vec!["--flag".to_string()],
                    envs: base_envs,
                },
            })
        }
    }

    struct SharedRunnerPlugin(Arc<dyn RunnerPlugin>);

    #[async_trait]
    impl RunnerPlugin for SharedRunnerPlugin {
        fn name(&self) -> &str {
            self.0.name()
        }

        async fn start_session(
            &self,
            args: &RunnerStartArgs,
        ) -> anyhow::Result<Box<dyn RunnerSession>> {
            self.0.start_session(args).await
        }
    }

    struct DummyRunnerPlugin;

    #[async_trait]
    impl RunnerPlugin for DummyRunnerPlugin {
        fn name(&self) -> &str {
            "dummy-runner"
        }

        async fn start_session(
            &self,
            _args: &RunnerStartArgs,
        ) -> anyhow::Result<Box<dyn RunnerSession>> {
            Ok(Box::new(DummySession {
                stdin: Some(tokio::io::sink()),
                stdout: Some(tokio::io::empty()),
                stderr: Some(tokio::io::empty()),
            }))
        }
    }

    struct DummySession {
        stdin: Option<tokio::io::Sink>,
        stdout: Option<tokio::io::Empty>,
        stderr: Option<tokio::io::Empty>,
    }

    #[async_trait]
    impl RunnerSession for DummySession {
        fn stdin(&mut self) -> Option<Box<dyn tokio::io::AsyncWrite + Unpin + Send>> {
            self.stdin.take().map(|s| Box::new(s) as _)
        }

        fn stdout(&mut self) -> Option<Box<dyn tokio::io::AsyncRead + Unpin + Send>> {
            self.stdout.take().map(|s| Box::new(s) as _)
        }

        fn stderr(&mut self) -> Option<Box<dyn tokio::io::AsyncRead + Unpin + Send>> {
            self.stderr.take().map(|s| Box::new(s) as _)
        }

        async fn signal(&mut self, _signal: Signal) -> anyhow::Result<()> {
            Ok(())
        }

        async fn wait(&mut self) -> anyhow::Result<RunOutcome> {
            Ok(RunOutcome {
                exit_code: 0,
                duration_ms: None,
                stdout_tail: String::new(),
                stderr_tail: String::new(),
                tool_events: vec![],
                shown_qa_ids: vec![],
                used_qa_ids: vec![],
            })
        }
    }

    struct DummyMemory;

    #[async_trait]
    impl MemoryPlugin for DummyMemory {
        fn name(&self) -> &str {
            "dummy-memory"
        }

        async fn search(
            &self,
            _payload: crate::memory::models::QASearchPayload,
        ) -> anyhow::Result<Vec<SearchMatch>> {
            Ok(vec![SearchMatch {
                qa_id: "qa-1".to_string(),
                project_id: None,
                question: "Q?".to_string(),
                answer: "A.".to_string(),
                tags: vec![],
                score: 0.9,
                relevance: 0.9,
                validation_level: 3,
                level: None,
                trust: 0.9,
                freshness: 1.0,
                confidence: 1.0,
                status: "active".to_string(),
                summary: None,
                source: None,
                expiry_at: None,
                metadata: Value::Null,
            }])
        }

        async fn record_hit(
            &self,
            _payload: crate::memory::models::QAHitsPayload,
        ) -> anyhow::Result<()> {
            Ok(())
        }

        async fn record_candidate(
            &self,
            _payload: crate::memory::models::QACandidatePayload,
        ) -> anyhow::Result<()> {
            Ok(())
        }

        async fn record_validation(
            &self,
            _payload: crate::memory::models::QAValidationPayload,
        ) -> anyhow::Result<()> {
            Ok(())
        }
    }

    struct DummyGatekeeper;

    impl GatekeeperPlugin for DummyGatekeeper {
        fn name(&self) -> &str {
            "dummy-gatekeeper"
        }

        fn evaluate(
            &self,
            _now: chrono::DateTime<chrono::Utc>,
            _matches: &[SearchMatch],
            _outcome: &RunOutcome,
            _events: &[crate::tool_event::ToolEvent],
        ) -> GatekeeperDecision {
            GatekeeperDecision {
                inject_list: vec![InjectItem {
                    qa_id: "qa-1".to_string(),
                    question: "Q?".to_string(),
                    answer: "A.".to_string(),
                    summary: None,
                    trust: 1.0,
                    validation_level: 3,
                    score: 1.0,
                    tags: vec![],
                }],
                should_write_candidate: false,
                hit_refs: vec![],
                validate_plans: vec![],
                reasons: vec![],
                signals: Value::Null,
            }
        }
    }

    #[tokio::test]
    async fn backend_plan_receives_memory_injected_prompt() {
        let last_prompt = Arc::new(Mutex::new(None));
        let runner: Arc<dyn RunnerPlugin> = Arc::new(DummyRunnerPlugin);

        let cfg = AppConfig::default();

        let (runner_spec, memory, gatekeeper) = (
            RunnerSpec::Backend {
                strategy: Box::new(CaptureBackendStrategy::new(last_prompt.clone(), runner)),
                backend_spec: "dummy".to_string(),
                base_envs: std::env::vars().collect(),
                resume_id: None,
                model: None,
                stream: false,
                stream_format: "text".to_string(),
            },
            Some(Arc::new(DummyMemory) as Arc<dyn MemoryPlugin>),
            Arc::new(DummyGatekeeper) as Arc<dyn GatekeeperPlugin>,
        );

        let exit = run_with_query(
            RunWithQueryArgs {
                user_query: "hello".to_string(),
                cfg,
                runner: runner_spec,
                run_id: "run-1".to_string(),
                capture_bytes: 128,
                silent: true,
                events_out_tx: None,
                policy: None,
                memory,
                gatekeeper,
                wrapper_start_data: None,
            },
            |input| async move {
                let _ = input; // session was started; we just return a synthetic result
                Ok(RunnerResult {
                    run_id: "run-1".to_string(),
                    exit_code: 0,
                    duration_ms: Some(1),
                    stdout_tail: String::new(),
                    stderr_tail: String::new(),
                    tool_events: vec![],
                    dropped_lines: 0,
                })
            },
        )
        .await
        .unwrap();
        assert_eq!(exit, 0);

        let prompt = last_prompt.lock().unwrap().clone().unwrap();
        assert!(prompt.contains("[MEMORY_CONTEXT v1]"));
        assert!(prompt.contains("[QA_REF qa-1]"));
        assert!(prompt.contains("hello"));
    }
}
