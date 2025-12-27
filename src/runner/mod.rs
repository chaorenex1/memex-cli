mod exit;
mod outcome;
mod spawn;
mod tee;

use std::collections::HashMap;
use std::time::{Duration, Instant};

use serde::Serialize;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;

use crate::cli::Args;
use crate::error::RunnerError;
use crate::config::ControlConfig;
use crate::events_out::EventsOutTx;

use crate::tool_event::ToolEvent;
use crate::tool_event::PrefixedJsonlParser;
use crate::tool_event::ToolEventRuntime;
use crate::util::RingBytes;

pub use outcome::RunOutcome;

pub struct RunnerResult {
    pub exit_code: i32,
    pub duration_ms: Option<u64>,
    pub stdout_tail: String,
    pub stderr_tail: String,
    pub tool_events: Vec<ToolEvent>,
    pub dropped_lines: u64,
}

pub async fn run_child_process(
    args: &Args,
    control: &ControlConfig,
    events_out: Option<EventsOutTx>,
    run_id: &str,
    stream_format: &str,
) -> Result<RunnerResult, RunnerError> {
    let mut child = spawn::spawn(args)?;

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let stdin = child.stdin.take().unwrap();

    let ring_out = RingBytes::new(args.capture_bytes);
    let ring_err = RingBytes::new(args.capture_bytes);

    let silent = stream_format == "jsonl";

    let (line_tx, mut line_rx) = mpsc::channel::<tee::LineTap>(1024);
    let out_task = tee::pump_stdout(stdout, ring_out.clone(), line_tx.clone(), silent);
    let err_task = tee::pump_stderr(stderr, ring_err.clone(), line_tx, silent);

    let (ctl_tx, mut ctl_rx) = mpsc::channel::<serde_json::Value>(128);
    let mut ctl = ControlChannel::new(stdin);
    let fail_closed = control.fail_mode.as_str() == "closed";

    let (writer_err_tx, mut writer_err_rx) = mpsc::channel::<String>(1);
    let ctl_task = tokio::spawn(async move {
        while let Some(v) = ctl_rx.recv().await {
            if let Err(e) = ctl.send(&v).await {
                let _ = writer_err_tx
                    .send(format!("stdin write failed: {}", e))
                    .await;
                break;
            }
        }
    });

    let pending: HashMap<String, Instant> = HashMap::new();
    let decision_timeout = Duration::from_millis(control.decision_timeout_ms);
    let mut tick = tokio::time::interval(Duration::from_millis(1000));

    let parser = PrefixedJsonlParser::new("@@MEM_TOOL_EVENT@@");
    let mut tool_runtime = ToolEventRuntime::new(parser, events_out.clone());

    let (exit_status, abort_reason) = {
        let wait_fut = child.wait();
        tokio::pin!(wait_fut);

        let mut status = None;
        let mut reason = None;

        loop {
            tokio::select! {
                res = &mut wait_fut => {
                    status = Some(res.map_err(|e: std::io::Error| RunnerError::Spawn(e.to_string())));
                    break;
                }

                maybe_err = writer_err_rx.recv() => {
                    if let Some(msg) = maybe_err {
                        tracing::error!(error.kind="control.stdin_broken", error.message=%msg);
                        if fail_closed {
                            reason = Some("control channel broken");
                            break;
                        } else {
                            tracing::warn!("control channel broken, continuing in fail-open mode");
                        }
                    }
                }

                tap = line_rx.recv() => {
                    if let Some(tap) = tap {
                        tool_runtime.observe_line(&tap.line).await;
                    }
                }

                _ = tick.tick() => {
                     let now = Instant::now();
                     let mut timed_out = false;
                     for (_, t0) in pending.iter() {
                         if now.duration_since(*t0) > decision_timeout {
                             timed_out = true;
                             break;
                         }
                     }
                     if timed_out {
                         tracing::error!(error.kind="control.decision_timeout");
                         if fail_closed {
                             reason = Some("decision timeout");
                             break;
                         }
                     }
                }
            }
        }
        (status, reason)
    };

    if let Some(reason) = abort_reason {
        abort_sequence(&mut child, &ctl_tx, run_id, control.abort_grace_ms, reason).await;
        return Ok(RunnerResult {
            exit_code: 40,
            duration_ms: None,
            stdout_tail: String::new(),
            stderr_tail: String::new(),
            tool_events: vec![],
            dropped_lines: tool_runtime.dropped_events_out(),
        });
    }

    drop(ctl_tx);
    ctl_task.abort();
    out_task.await.ok();
    err_task.await.ok();

    let status = exit_status.unwrap()?;
    let exit_code = exit::normalize_exit(status);
    let start_time = Instant::now();

    let stdout_tail = String::from_utf8_lossy(&ring_out.to_bytes()).to_string();
    let stderr_tail = String::from_utf8_lossy(&ring_err.to_bytes()).to_string();

    let tool_events = tool_runtime.take_events();
    let dropped = tool_runtime.dropped_events_out();

    Ok(RunnerResult {
        exit_code,
        duration_ms: Some(start_time.elapsed().as_millis() as u64),
        stdout_tail,
        stderr_tail,
        tool_events,
        dropped_lines: dropped,
    })
}
async fn abort_sequence(
    child: &mut tokio::process::Child,
    ctl_tx: &mpsc::Sender<serde_json::Value>,
    run_id: &str,
    abort_grace_ms: u64,
    reason: &str,
) {
    let abort = PolicyAbortCmd::new(run_id.to_string(), reason.to_string(), Some("policy_violation".into()));
    let _ = ctl_tx.send(serde_json::to_value(abort).unwrap()).await;
    tokio::time::sleep(Duration::from_millis(abort_grace_ms)).await;
    let _ = child.kill().await;
}

pub struct ControlChannel {
    stdin: tokio::process::ChildStdin,
}

impl ControlChannel {
    pub fn new(stdin: tokio::process::ChildStdin) -> Self {
        Self { stdin }
    }

    pub async fn send<T: Serialize>(&mut self, msg: &T) -> std::io::Result<()> {
        let line = serde_json::to_string(msg).unwrap();
        self.stdin.write_all(line.as_bytes()).await?;
        self.stdin.write_all(b"\n").await?;
        self.stdin.flush().await
    }
}

#[derive(Debug, Serialize)]
struct PolicyAbortCmd {
    pub v: u8,
    #[serde(rename = "type")]
    pub ty: &'static str,
    pub ts: String,
    pub run_id: String,
    pub id: String,
    pub reason: String,
    pub code: Option<String>,
}

impl PolicyAbortCmd {
    pub fn new(run_id: String, reason: String, code: Option<String>) -> Self {
        Self {
            v: 1,
            ty: "policy.abort",
            ts: chrono::Utc::now().to_rfc3339(),
            run_id,
            id: "abort-1".into(),
            reason,
            code,
        }
    }
}

