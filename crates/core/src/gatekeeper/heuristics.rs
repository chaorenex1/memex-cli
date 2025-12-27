use async_trait::async_trait;
use serde_json::json;
use crate::gatekeeper::r#trait::{Gatekeeper, GatekeeperInput, GatekeeperDecision};
use crate::memory::r#trait::{CandidateRequest, ValidateRequest};
use crate::types::ProjectId;

pub struct SimpleGatekeeper;

#[async_trait]
impl Gatekeeper for SimpleGatekeeper {
    async fn evaluate(&self, input: GatekeeperInput) -> anyhow::Result<GatekeeperDecision> {
        let mut reasons = Vec::new();
        let mut signals = json!({});

        // 1. Analyze Signal Strength (Heuristic)
        let (signal_strength, strong_signal, strength_label) = calculate_signal_strength(&input.user_query);
        reasons.push(format!("Signal strength: {} ({})", strength_label, signal_strength));
        
        // 2. Analyze Result
        let success = input.exit_code == 0;
        let result_str = if success { "pass" } else { "fail" };
        reasons.push(format!("Execution result: {} (code {})", result_str, input.exit_code));

        // 3. Validation Logic
        // If we injected items, we should validate them with the result of this run.
        let mut validations = Vec::new();
        if !input.injected_items.items.is_empty() {
            for item in &input.injected_items.items {
                // For MVP: Validate all injected items as relevant context.
                // In future: Check if they were actually used/hit.
                validations.push(ValidateRequest {
                    project_id: ProjectId("default".to_string()), // Should ideally come from input or config
                    qa_id: item.qa_id.clone(),
                    result: Some(result_str.to_string()),
                    signal_strength: Some(signal_strength),
                    success: Some(success),
                    strong_signal: Some(strong_signal),
                    source: Some("qa-run".to_string()),
                    context: Some(json!({
                        "command": input.user_query,
                        "exit_code": input.exit_code,
                        "duration_ms": input.duration_ms
                    }).to_string()),
                    client: Some("mem-codecli".to_string()),
                    message_id: None,
                    payload: None,
                });
            }
        }

        // 4. Candidate Generation Logic
        // Only generate candidate if:
        // - Execution succeeded
        // - No strong match in injected items (avoid duplication) - threshold e.g., 0.85
        // - Output has content
        
        let mut should_write_candidate = false;
        let mut candidate = None;

        let max_score = input.injected_items.items.iter()
            .map(|item| item.score)
            .fold(0.0f32, |a, b| a.max(b));

        if success && max_score < 0.85 && (!input.final_stdout.is_empty() || !input.final_stderr.is_empty()) {
            // Heuristic: If command contains "test" or "build", maybe the output is logs, not a Q&A.
            // But for "how to", the command IS the question.
            
            should_write_candidate = true;
            candidate = Some(CandidateRequest {
                project_id: ProjectId("default".to_string()),
                question: input.user_query.clone(),
                answer: format!("Command executed successfully.\n\nStdout:\n{}\n\nStderr:\n{}", 
                    input.final_stdout.trim(), 
                    input.final_stderr.trim()
                ),
                summary: None,
                tags: vec!["auto-generated".to_string(), strength_label.to_string()],
                confidence: 0.5, // Default confidence for auto-capture
                metadata: json!({
                    "source": "mem-codecli",
                    "exit_code": input.exit_code,
                    "duration_ms": input.duration_ms
                }),
                source: Some("mem-codecli".to_string()),
                author: None,
            });
            reasons.push("Candidate generated: success & low duplicate score".to_string());
        } else {
             reasons.push(format!("Candidate skipped: success={}, max_score={:.2}", success, max_score));
        }

        Ok(GatekeeperDecision {
            should_write_candidate,
            candidate,
            should_validate: !validations.is_empty(),
            validate: validations,
            reasons,
            signals,
        })
    }
}

fn calculate_signal_strength(cmd: &str) -> (f32, bool, &'static str) {
    let lower = cmd.to_lowercase();
    // Strong signals: tests, builds
    if lower.contains("test") || lower.contains("pytest") || lower.contains("npm test") || lower.contains("cargo test") || lower.contains("go test") {
        return (1.0, true, "strong");
    }
    if lower.contains("build") || lower.contains("compile") || lower.contains("cargo build") || lower.contains("npm run build") {
        return (1.0, true, "strong");
    }
    
    // Medium signals: scripts
    if lower.contains(".sh") || lower.contains(".py") || lower.contains("node ") || lower.contains("python ") {
        return (0.5, false, "medium");
    }
    
    // Weak signals: misc
    (0.1, false, "weak")
}
