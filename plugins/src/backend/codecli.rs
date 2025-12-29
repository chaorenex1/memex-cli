use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;

use memex_core::api as core_api;

use crate::runner::codecli::CodeCliRunnerPlugin;

pub struct CodeCliBackendStrategy;

impl core_api::BackendStrategy for CodeCliBackendStrategy {
    fn name(&self) -> &str {
        "codecli"
    }

    fn plan(
        &self,
        backend: &str,
        base_envs: HashMap<String, String>,
        resume_id: Option<String>,
        prompt: String,
        model: Option<String>,
        stream: bool,
        stream_format: &str,
    ) -> Result<core_api::BackendPlan> {
        tracing::info!(
            "Planning CodeCli backend '{}' with resume_id={:?}, model={:?}, stream={}, stream_format={},base_envs={:?}",
            backend,
            resume_id,
            model,
            stream,
            stream_format,
            base_envs
        );

        let mut args: Vec<String> = Vec::new();

        let exe = backend_basename_lower(backend);
        let want_stream_json = stream_format == "jsonl";

        if exe.contains("codex") {
            // Matches examples like: codex exec "..." --json
            args.push("exec".to_string());

            if let Some(m) = &model {
                args.push("--model".to_string());
                args.push(m.clone());
            }

            if want_stream_json {
                args.push("--json".to_string());
            }

            // Resume: codex exec [--json] resume <id> <prompt>
            if let Some(resume_id) = resume_id.as_deref() {
                if !resume_id.trim().is_empty() {
                    args.push("resume".to_string());
                    args.push(resume_id.to_string());
                }
            }

            if !prompt.is_empty() {
                args.push(prompt);
            }
        } else if exe.contains("claude") {
            // Matches examples like:
            // claude "..." -p --output-format stream-json --verbose
            if !prompt.is_empty() {
                args.push(prompt);
            }

            if stream || want_stream_json {
                args.push("-p".to_string());
            }

            if want_stream_json {
                args.push("--output-format".to_string());
                args.push("stream-json".to_string());
            }

            if let Some(m) = &model {
                args.push("--model".to_string());
                args.push(m.clone());
            }

            // Resume: -r <id>
            if let Some(resume_id) = resume_id.as_deref() {
                if !resume_id.trim().is_empty() {
                    args.push("-r".to_string());
                    args.push(resume_id.to_string());
                }
            }
        } else if exe.contains("gemini") {
            // Matches examples like:
            // gemini -p "..." -y -o stream-json
            if !prompt.is_empty() {
                args.push("-p".to_string());
                args.push(prompt);
            }

            if want_stream_json {
                args.push("-o".to_string());
                args.push("stream-json".to_string());
            }

            // Resume: -r <id> (e.g. -r latest)
            if let Some(resume_id) = resume_id.as_deref() {
                if !resume_id.trim().is_empty() {
                    args.push("-r".to_string());
                    args.push(resume_id.to_string());
                }
            }

            // Leave -y (YOLO) and auth concerns to the user's environment.
            if let Some(m) = &model {
                args.push("--model".to_string());
                args.push(m.clone());
            }
        } else {
            // Generic passthrough-ish fallback (previous behavior).
            if let Some(m) = model {
                args.push("--model".to_string());
                args.push(m);
            }
            if stream {
                args.push("--stream".to_string());
            }
            if !prompt.is_empty() {
                args.push(prompt);
            }
        }

        Ok(core_api::BackendPlan {
            runner: Box::new(CodeCliRunnerPlugin::new()),
            session_args: core_api::RunnerStartArgs {
                cmd: backend.to_string(),
                args,
                envs: base_envs,
            },
        })
    }
}

fn backend_basename_lower(backend: &str) -> String {
    let p = Path::new(backend);
    let s = p.file_stem().and_then(|x| x.to_str()).unwrap_or(backend);
    s.to_ascii_lowercase()
}
