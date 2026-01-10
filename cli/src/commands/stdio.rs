use std::fs;
use std::io::Read;

use memex_core::api as core_api;
use memex_plugins::plan::{build_runner_spec, PlanMode, PlanRequest};
use uuid::Uuid;

use crate::commands::cli::StdioArgs;

pub async fn handle_stdio(
    args: StdioArgs,
    capture_bytes: usize,
    ctx: &core_api::AppContext,
) -> Result<i32, core_api::CliError> {
    if args.quiet && args.verbose {
        return Err(core_api::CliError::Command(
            "--quiet and --verbose are mutually exclusive".to_string(),
        ));
    }

    if args.run_id.is_some() && args.events_file.is_none() {
        return Err(core_api::CliError::Command(
            "--run-id requires --events-file".to_string(),
        ));
    }

    if args.events_file.is_some() && args.run_id.is_none() {
        return Err(core_api::CliError::Command(
            "--events-file requires --run-id".to_string(),
        ));
    }

    let input = match args.input_file.as_deref() {
        Some(path) => fs::read_to_string(path).map_err(core_api::CliError::Io)?,
        None => {
            let mut buf = String::new();
            std::io::stdin()
                .read_to_string(&mut buf)
                .map_err(core_api::CliError::Io)?;
            buf
        }
    };

    let mut tasks = match core_api::parse_stdio_tasks(&input) {
        Ok(t) => t,
        Err(e) => {
            let code = e.error_code().as_u16() as i32;
            emit_stdio_error(&args, &e, None);
            return Ok(code);
        }
    };

    // Load resume context if provided
    let resume_context =
        if let (Some(run_id), Some(events_file)) = (&args.run_id, &args.events_file) {
            match load_resume_context(events_file, run_id) {
                Ok(ctx) => Some(ctx),
                Err(e) => {
                    eprintln!("Failed to load resume context: {}", e);
                    return Ok(1);
                }
            }
        } else {
            None
        };

    // CLI stream-format overrides per-task defaults to keep output consistent.
    for t in tasks.iter_mut() {
        t.stream_format = args.stream_format.clone();
    }

    let exec_args = core_api::StdioRunOpts {
        stream_format: args.stream_format.clone(),
        ascii: args.ascii,
        verbose: args.verbose,
        quiet: args.quiet,
        capture_bytes,
        resume_run_id: args.run_id.clone(),
        resume_context,
    };

    let planner = |task: &core_api::StdioTask| -> Result<
        (core_api::RunnerSpec, Option<serde_json::Value>),
        core_api::StdioError,
    > {
        let mut cfg = ctx.cfg().clone();
        let plan_req = PlanRequest {
            mode: PlanMode::Backend {
                backend_spec: task.backend.clone(),
                backend_kind: None,
                env_file: None,
                env: vec![],
                model: task.model.clone(),
                model_provider: task.model_provider.clone(),
                project_id: Some(task.workdir.clone()),
                task_level: None,
            },
            resume_id: None,
            stream_format: task.stream_format.clone(),
        };
        let (runner_spec, start_data) = build_runner_spec(&mut cfg, plan_req)
            .map_err(|e| core_api::StdioError::BackendError(e.to_string()))?;
        Ok((runner_spec, start_data))
    };

    match core_api::run_stdio(tasks, ctx, &exec_args, planner).await {
        Ok(code) => Ok(code),
        Err(e) => {
            let code = e.error_code().as_u16() as i32;
            emit_stdio_error(&args, &e, Some(Uuid::new_v4().to_string()));
            Ok(code)
        }
    }
}

fn load_resume_context(events_file: &str, run_id: &str) -> Result<String, String> {
    use std::io::BufRead;

    let file = std::fs::File::open(events_file)
        .map_err(|e| format!("Failed to open events file: {}", e))?;
    let reader = std::io::BufReader::new(file);

    let mut context_lines = Vec::new();
    let mut found_run = false;

    for line in reader.lines() {
        let line = line.map_err(|e| format!("Failed to read line: {}", e))?;

        // Parse JSONL
        if let Ok(ev) = serde_json::from_str::<serde_json::Value>(&line) {
            if let Some(rid) = ev.get("run_id").and_then(|v| v.as_str()) {
                if rid == run_id {
                    found_run = true;

                    // Collect relevant events
                    if let Some(event_type) = ev.get("type").and_then(|v| v.as_str()) {
                        match event_type {
                            "assistant.output" | "assistant.thinking" | "assistant.action" => {
                                if let Some(output) = ev.get("output").and_then(|v| v.as_str()) {
                                    context_lines.push(output.to_string());
                                }
                            }
                            "tool.result" => {
                                if let Some(action) = ev.get("action").and_then(|v| v.as_str()) {
                                    if let Some(output) = ev.get("output").and_then(|v| v.as_str())
                                    {
                                        context_lines
                                            .push(format!("[Tool: {}]\n{}", action, output));
                                    }
                                }
                            }
                            "run.end" => break,
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    if !found_run {
        return Err(format!("Run ID {} not found in events file", run_id));
    }

    if context_lines.is_empty() {
        return Ok(String::new());
    }

    Ok(format!(
        "=== Previous Context (run_id: {}) ===\n{}\n=== End Previous Context ===\n\n",
        run_id,
        context_lines.join("\n")
    ))
}

fn emit_stdio_error(args: &StdioArgs, err: &core_api::StdioError, run_id: Option<String>) {
    let id = run_id.unwrap_or_else(|| Uuid::new_v4().to_string());
    if args.stream_format == "jsonl" {
        let code = err.error_code().as_u16() as i32;
        let ev = core_api::JsonlEvent {
            v: 1,
            event_type: "error".to_string(),
            ts: chrono::Utc::now().to_rfc3339(),
            run_id: id,
            task_id: None,
            action: None,
            args: None,
            output: None,
            error: Some(err.to_string()),
            code: Some(code),
            progress: None,
            metadata: None,
        };
        core_api::emit_stdio_json(&ev);

        let end = core_api::JsonlEvent {
            v: 1,
            event_type: "run.end".to_string(),
            ts: chrono::Utc::now().to_rfc3339(),
            run_id: ev.run_id.clone(),
            task_id: None,
            action: None,
            args: None,
            output: None,
            error: None,
            code: Some(code),
            progress: Some(100),
            metadata: Some(serde_json::json!({ "status": "failed" })),
        };
        core_api::emit_stdio_json(&end);
    } else {
        let marker = if args.ascii { "[FAIL]" } else { "✗" };
        eprintln!("{} {}", marker, err);
    }
}
