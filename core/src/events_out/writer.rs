use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;

use crate::config::EventsOutConfig;

fn audit_preview(s: &str) -> String {
    const MAX: usize = 120;
    if s.len() <= MAX {
        return s.to_string();
    }
    let end = s
        .char_indices()
        .take_while(|(i, _)| *i < MAX)
        .last()
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(0);
    let mut out = s[..end].to_string();
    out.push('…');
    out
}

#[derive(Clone)]
pub struct EventsOutTx {
    tx: mpsc::Sender<String>,
    dropped: std::sync::Arc<std::sync::atomic::AtomicU64>,
    drop_when_full: bool,
}

impl EventsOutTx {
    pub fn dropped_count(&self) -> u64 {
        self.dropped.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub async fn send_line(&self, line: String) {
        if self.drop_when_full {
            match self.tx.try_send(line) {
                Ok(_) => {}
                Err(_) => {
                    let count = self.dropped.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    // Log every 100 dropped events to avoid log spam
                    if count.is_multiple_of(100) {
                        tracing::warn!(
                            target: "memex.events_out",
                            dropped_total = count,
                            "events_out channel full, messages are being dropped"
                        );
                    }
                }
            }
        } else if self.tx.send(line).await.is_err() {
            tracing::debug!(
                target: "memex.events_out",
                "events_out writer closed, send failed"
            );
        }
    }
}

pub async fn start_events_out(cfg: &EventsOutConfig) -> Result<Option<EventsOutTx>, String> {
    // Explicit checks with logging to help diagnose why events_out might be disabled
    if !cfg.enabled {
        tracing::warn!(
            target: "memex.events_out",
            "events_out is disabled in config (enabled=false), no tool events will be written to file"
        );
        return Ok(None);
    }
    if cfg.path.trim().is_empty() {
        tracing::warn!(
            target: "memex.events_out",
            "events_out path is empty in config, no tool events will be written to file"
        );
        return Ok(None);
    }

    tracing::info!(
        target: "memex.events_out",
        path = %cfg.path,
        channel_capacity = cfg.channel_capacity,
        drop_when_full = cfg.drop_when_full,
        "events_out writer started"
    );

    let (tx, mut rx) = mpsc::channel::<String>(cfg.channel_capacity);
    let dropped = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    let dropped_clone = dropped.clone();
    let path = cfg.path.clone();
    let drop_when_full = cfg.drop_when_full;

    tokio::spawn(async move {
        let mut writer: Box<dyn tokio::io::AsyncWrite + Unpin + Send> = if path == "stdout:" {
            Box::new(tokio::io::stdout())
        } else {
            let file = match tokio::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .await
            {
                Ok(f) => f,
                Err(_) => return,
            };
            Box::new(file)
        };

        let mut write_count = 0usize;
        while let Some(mut line) = rx.recv().await {
            if !line.ends_with('\n') {
                line.push('\n');
            }
            if path == "stdout:" {
                tracing::debug!(
                    target: "memex.stdout_audit",
                    kind = "events_out",
                    bytes = line.len(),
                    preview = %audit_preview(line.trim_end())
                );
            }
            // Debug: log first few writes to verify newline handling
            if write_count < 5 {
                tracing::debug!(
                    target: "memex.events_out",
                    count = write_count,
                    has_newline = line.ends_with('\n'),
                    bytes = line.len(),
                    preview = %audit_preview(line.trim_end()),
                    "writing line to events_out file"
                );
            }
            if writer.write_all(line.as_bytes()).await.is_err() {
                tracing::error!(
                    target: "memex.events_out",
                    "failed to write to events_out file, writer task exiting"
                );
                return;
            }
            write_count += 1;
            // Flush periodically to ensure data is written to disk
            // Every 10 writes or for stdout, flush immediately
            if write_count % 10 == 0 || path == "stdout:" {
                if writer.flush().await.is_err() {
                    tracing::error!(
                        target: "memex.events_out",
                        "failed to flush events_out file"
                    );
                    return;
                }
            }
        }

        let _ = writer.flush().await;
        let _ = dropped_clone.load(std::sync::atomic::Ordering::Relaxed);
    });

    Ok(Some(EventsOutTx {
        tx,
        dropped,
        drop_when_full,
    }))
}
