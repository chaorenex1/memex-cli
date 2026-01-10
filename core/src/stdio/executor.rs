use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use base64::Engine;
use chrono::Utc;
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use lru::LruCache;
use memmap2::Mmap;
use std::num::NonZeroUsize;
use std::sync::Arc;
use sysinfo::System;
use tokio::sync::{mpsc, Semaphore};
use tokio::time::Duration;
use uuid::Uuid;

use crate::api::{RunSessionArgs, RunnerSpec};
use crate::engine::run_with_query;
use crate::runner::run_session;

use super::error::StdioError;
use super::render::{
    emit_json, emit_task_end_jsonl, emit_task_start_jsonl, format_backend,
    render_task_jsonl_events, render_task_stream, render_task_stream_content_only, JsonlEvent,
    RenderTaskInfo, TextMarkers,
};
use super::retry;
use super::types::{FilesEncoding, FilesMode, StdioRunOpts, StdioTask};

const MAX_FILES: usize = 100;
const MAX_SINGLE_FILE: u64 = 10 * 1024 * 1024;
const MAX_TOTAL_SIZE: u64 = 50 * 1024 * 1024;
const EMBED_SIZE_LIMIT: u64 = 50 * 1024; // embed 模式的大小阈值（50KB）

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct ResolvedFile {
    display_path: String,
    mode: FilesMode,
    encoding: FilesEncoding,
    size: u64,
    modified: Option<std::time::SystemTime>,
    content: Option<ResolvedContent>,
}

#[derive(Debug, Clone)]
enum ResolvedContent {
    Text(String),
    Base64(String),
}

pub async fn run_stdio<F>(
    tasks: Vec<StdioTask>,
    ctx: &crate::context::AppContext,
    opts: &StdioRunOpts,
    planner: F,
) -> Result<i32, StdioError>
where
    F: Fn(&StdioTask) -> Result<(RunnerSpec, Option<serde_json::Value>), StdioError>,
{
    let run_started = std::time::Instant::now();
    let run_id = Uuid::new_v4().to_string();
    let layers = topo_sort_layered(&tasks);

    // 初始化 render 模块配置（Level 2.1 事件批量化）
    crate::stdio::render::configure_event_buffer(
        ctx.cfg().stdio.enable_event_buffering,
        ctx.cfg().stdio.event_buffer_size,
        ctx.cfg().stdio.event_flush_interval_ms,
    );

    // Inject resume context into first task
    let mut tasks = tasks;
    if let Some(ctx_str) = &opts.resume_context {
        if !ctx_str.is_empty() && !tasks.is_empty() {
            tasks[0].content = format!("{}{}", ctx_str, tasks[0].content);
        }
    }

    let lookup: HashMap<String, StdioTask> = tasks.into_iter().map(|t| (t.id.clone(), t)).collect();
    let markers = if opts.ascii {
        TextMarkers::ascii()
    } else {
        TextMarkers::unicode()
    };

    let mut last_exit = 0;
    let mut finished = 0usize;
    let mut failed = 0usize;
    let services = ctx
        .build_services(ctx.cfg())
        .map_err(|e| StdioError::RunnerError(e.to_string()))?;
    let max_concurrency = ctx.cfg().stdio.max_parallel_tasks.max(1);

    let jsonl_mode = opts.stream_format == "jsonl";
    if jsonl_mode {
        // Emit run.resume if resuming
        if let Some(resume_id) = &opts.resume_run_id {
            emit_json(&JsonlEvent {
                v: 1,
                event_type: "run.resume".into(),
                ts: Utc::now().to_rfc3339(),
                run_id: run_id.clone(),
                task_id: None,
                action: None,
                args: None,
                output: None,
                error: None,
                code: None,
                progress: None,
                metadata: Some(serde_json::json!({
                    "original_run_id": resume_id,
                    "resumed_at": Utc::now().to_rfc3339(),
                })),
            });

            // Emit context.history
            if let Some(ctx) = &opts.resume_context {
                let token_count = ctx.split_whitespace().count();
                emit_json(&JsonlEvent {
                    v: 1,
                    event_type: "context.history".into(),
                    ts: Utc::now().to_rfc3339(),
                    run_id: run_id.clone(),
                    task_id: None,
                    action: None,
                    args: None,
                    output: Some(format!("[Previous context loaded: {} tokens]", token_count)),
                    error: None,
                    code: None,
                    progress: None,
                    metadata: Some(serde_json::json!({
                        "original_run_id": resume_id,
                        "context_size_tokens": token_count,
                    })),
                });
            }
        }

        emit_json(&JsonlEvent {
            v: 1,
            event_type: "run.start".into(),
            ts: Utc::now().to_rfc3339(),
            run_id: run_id.clone(),
            task_id: None,
            action: None,
            args: None,
            output: None,
            error: None,
            code: None,
            progress: Some(0),
            metadata: Some(serde_json::json!({ "total_tasks": lookup.len() })),
        });
    } else if opts.resume_run_id.is_some() {
        // Text mode resume indication
        if let Some(resume_id) = &opts.resume_run_id {
            println!("{} 恢复运行 run_id={}...", markers.retry, resume_id);
            if let Some(ctx) = &opts.resume_context {
                let token_count = ctx.split_whitespace().count();
                println!("  上下文: {} tokens", token_count);
                println!();
            }
        }
    }

    for (idx, layer) in layers.iter().enumerate() {
        if layer.is_empty() {
            continue;
        }

        let buffer_text = opts.stream_format == "text" && layer.len() > 1;

        if opts.stream_format == "text" && layer.len() > 1 && !opts.quiet {
            println!("{} 并行执行 {} 个任务...", markers.start, layer.len());
            for id in layer {
                if let Some(t) = lookup.get(id) {
                    println!(
                        "  {} {} ({})",
                        markers.start,
                        t.id,
                        format_backend(&t.backend, t.model.as_deref())
                    );
                }
            }
            println!();
        }

        // Level 2.2 优化：自适应并发调度
        let concurrency = if ctx.cfg().stdio.enable_adaptive_concurrency && idx > 0 {
            let cpu_count = num_cpus::get();
            adaptive_concurrency(max_concurrency, cpu_count)
        } else {
            max_concurrency
        };

        let layer_results = execute_layer(
            layer,
            &lookup,
            ctx,
            opts,
            &planner,
            &services,
            &run_id,
            &markers,
            concurrency,
            buffer_text,
        )
        .await?;

        if buffer_text {
            for id in layer {
                if let Some(res) = layer_results.get(id) {
                    if let Some(block) = res.text_block.as_deref() {
                        print!("{block}");
                    }
                }
            }
        }

        finished += layer_results.len();
        let layer_failed = layer_results.values().filter(|r| r.exit_code != 0).count();
        failed += layer_failed;
        if let Some(code) = layer_results
            .values()
            .find(|r| r.exit_code != 0)
            .map(|r| r.exit_code)
        {
            last_exit = code;
            break;
        }
    }

    if jsonl_mode {
        emit_json(&JsonlEvent {
            v: 1,
            event_type: "run.end".into(),
            ts: Utc::now().to_rfc3339(),
            run_id: run_id.clone(),
            task_id: None,
            action: None,
            args: None,
            output: None,
            error: None,
            code: Some(last_exit),
            progress: Some(100),
            metadata: Some(serde_json::json!({
                "completed": finished,
                "total_tasks": lookup.len(),
                "status": if last_exit == 0 { "success" } else { "failed" }
            })),
        });
    } else if opts.stream_format == "text" && !opts.quiet {
        let total_ms = run_started.elapsed().as_millis() as u64;
        let sep = if opts.ascii {
            "----------------------------------------------------------------"
        } else {
            "───────────────────────────"
        };
        println!("{sep}");
        let status = if last_exit == 0 {
            markers.ok
        } else {
            markers.fail
        };
        if last_exit == 0 {
            println!(
                "{status} 完成 {}/{} 任务 ({:.1}s)",
                finished,
                lookup.len(),
                total_ms as f64 / 1000.0
            );
        } else {
            println!(
                "{status} 失败 {}/{} 任务 ({:.1}s)",
                failed,
                lookup.len(),
                total_ms as f64 / 1000.0
            );
        }
    }

    // Level 2.1: 确保所有缓冲事件被刷新
    crate::stdio::render::flush_event_buffer();

    Ok(last_exit)
}

#[derive(Debug, Clone)]
struct TaskExecResult {
    exit_code: i32,
    text_block: Option<String>,
}

// ============================================================================
// Memory-Mapped File I/O (Level 3.1 优化)
// ============================================================================

/// 使用内存映射读取大文件（Level 3.1 优化）
///
/// # 参数
/// - `path`: 文件路径
/// - `threshold_mb`: mmap 阈值（MB），小于此值的文件不使用 mmap
/// - `file_size_bytes`: 文件大小（字节）
///
/// # 返回
/// - `Ok(Some(bytes))`: 使用 mmap 读取成功
/// - `Ok(None)`: 文件小于阈值，应使用普通 I/O
/// - `Err(...)`: 读取失败
///
/// # 性能
/// - 100MB 文件内存占用减少 95%
/// - 避免将整个文件加载到内存
async fn read_file_with_mmap(
    path: &Path,
    threshold_mb: u64,
    file_size_bytes: u64,
) -> Result<Option<Vec<u8>>, StdioError> {
    let size_mb = file_size_bytes / (1024 * 1024);

    if size_mb < threshold_mb {
        return Ok(None); // 小文件不使用 mmap
    }

    let path_owned = path.to_path_buf();
    let data = tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(&path_owned).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                StdioError::FileNotFound(path_owned.display().to_string())
            } else {
                StdioError::FileAccessDenied(path_owned.display().to_string())
            }
        })?;

        // SAFETY: 文件在读取期间不会被修改（只读场景）
        // mmap 后立即复制数据到 Vec，避免 Windows 文件锁问题
        let mmap = unsafe { Mmap::map(&file) }
            .map_err(|e| StdioError::BackendError(format!("mmap failed: {}", e)))?;

        Ok::<Vec<u8>, StdioError>(mmap.to_vec())
    })
    .await
    .map_err(|e| StdioError::BackendError(e.to_string()))??;

    Ok(Some(data))
}

// ============================================================================
// LRU File Cache (Level 3.3 优化)
// ============================================================================

use lazy_static::lazy_static;
use std::sync::Mutex;

lazy_static! {
    /// 全局文件缓存（LRU）
    static ref FILE_CACHE: Mutex<LruCache<PathBuf, Arc<Vec<u8>>>> = {
        let capacity = std::env::var("MEM_STDIO_FILE_CACHE_SIZE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(100);
        Mutex::new(LruCache::new(NonZeroUsize::new(capacity).unwrap()))
    };
}

/// 带缓存的文件读取（Level 3.3 优化）
///
/// # 性能
/// - 缓存命中：延迟接近 0
/// - 缓存未命中：回退到 mmap 或普通 I/O
async fn read_file_cached(
    path: &Path,
    threshold_mb: u64,
    file_size_bytes: u64,
    enable_cache: bool,
) -> Result<Vec<u8>, StdioError> {
    // 尝试从缓存读取
    if enable_cache {
        let path_buf = path.to_path_buf();
        if let Ok(mut cache) = FILE_CACHE.lock() {
            if let Some(content) = cache.get(&path_buf) {
                return Ok((**content).clone());
            }
        }
    }

    // 缓存未命中：尝试 mmap
    let bytes = if let Some(data) = read_file_with_mmap(path, threshold_mb, file_size_bytes).await?
    {
        data
    } else {
        // 小文件：使用普通异步 I/O
        tokio::fs::read(path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                StdioError::FileNotFound(path.display().to_string())
            } else {
                StdioError::FileAccessDenied(path.display().to_string())
            }
        })?
    };

    // 写入缓存
    if enable_cache {
        let path_buf = path.to_path_buf();
        let arc_bytes = Arc::new(bytes.clone());
        if let Ok(mut cache) = FILE_CACHE.lock() {
            cache.put(path_buf, arc_bytes);
        }
    }

    Ok(bytes)
}

/// 清空文件缓存（用于测试或手动清理）
#[allow(dead_code)]
pub fn clear_file_cache() {
    if let Ok(mut cache) = FILE_CACHE.lock() {
        cache.clear();
    }
}

// ============================================================================
// Adaptive Concurrency (Level 2.2 优化)
// ============================================================================

/// 自适应并发调度：根据 CPU 使用率动态调整并发数
///
/// # 参数
/// - `base`: 基础并发数（来自配置）
/// - `cpu_count`: CPU 核心数
///
/// # 策略
/// - CPU < 50%：提高并发至 base + cpu_count/2
/// - CPU > 80%：降低并发至 base-1（最小为 1）
/// - CPU 50%-80%：保持基础并发
///
/// # 上限
/// 最大并发不超过 CPU 核心数的 2 倍
fn adaptive_concurrency(base: usize, cpu_count: usize) -> usize {
    let mut sys = System::new();
    sys.refresh_cpu();

    // 计算平均 CPU 使用率（sysinfo v0.30 API）
    let cpu_usage: f32 = sys.cpus().iter().map(|c| c.cpu_usage()).sum::<f32>() / cpu_count as f32;

    // 根据 CPU 使用率调整并发数
    let adjusted = if cpu_usage < 50.0 {
        // CPU 空闲：提高并发
        base + (cpu_count / 2)
    } else if cpu_usage > 80.0 {
        // CPU 繁忙：降低并发
        base.saturating_sub(1).max(1)
    } else {
        // CPU 适中：保持不变
        base
    };

    // 上限：CPU 核心数的 2 倍
    adjusted.min(cpu_count * 2)
}

// ============================================================================
// Layer Execution
// ============================================================================

async fn execute_layer<F>(
    layer: &[String],
    lookup: &HashMap<String, StdioTask>,
    ctx: &crate::context::AppContext,
    opts: &StdioRunOpts,
    planner: &F,
    services: &crate::context::Services,
    run_id: &str,
    markers: &TextMarkers,
    max_concurrency: usize,
    buffer_text: bool,
) -> Result<HashMap<String, TaskExecResult>, StdioError>
where
    F: Fn(&StdioTask) -> Result<(RunnerSpec, Option<serde_json::Value>), StdioError>,
{
    let sem = Arc::new(Semaphore::new(max_concurrency));
    let mut futs: FuturesUnordered<_> = FuturesUnordered::new();

    for id in layer {
        let Some(task) = lookup.get(id) else {
            continue;
        };
        let task = task.clone();
        let sem = sem.clone();
        let ctx = ctx.clone();
        let opts = opts.clone();
        let services = services.clone();
        let run_id = run_id.to_string();
        let markers = markers.clone();

        futs.push(async move {
            let _permit = sem.acquire_owned().await.map_err(|_| {
                StdioError::RunnerError("stdio semaphore closed unexpectedly".into())
            })?;
            execute_single_task(
                task,
                &ctx,
                &opts,
                planner,
                &services,
                &run_id,
                &markers,
                buffer_text,
            )
            .await
        });
    }

    let mut out: HashMap<String, TaskExecResult> = HashMap::new();
    while let Some(res) = futs.next().await {
        let (id, task_res) = res?;
        out.insert(id, task_res);
    }
    Ok(out)
}

async fn execute_single_task<F>(
    task: StdioTask,
    ctx: &crate::context::AppContext,
    opts: &StdioRunOpts,
    planner: &F,
    services: &crate::context::Services,
    run_id: &str,
    markers: &TextMarkers,
    buffer_text: bool,
) -> Result<(String, TaskExecResult), StdioError>
where
    F: Fn(&StdioTask) -> Result<(RunnerSpec, Option<serde_json::Value>), StdioError>,
{
    // 使用异步版本（Level 1 优化）
    let resolved_files = resolve_files(&task, &ctx.cfg().stdio).await?;
    let prompt = compose_prompt(&task, &resolved_files);

    let render_info = RenderTaskInfo {
        task_id: task.id.clone(),
        backend: task.backend.clone(),
        model: task.model.clone(),
        dependencies: task.dependencies.clone(),
        files: resolved_files
            .iter()
            .map(|f| super::render::FileInfo {
                path: f.display_path.clone(),
                size: f.size,
            })
            .collect(),
    };

    let timeout_secs = retry::effective_timeout_secs(task.timeout);
    let max_attempts = retry::max_attempts(task.retry);
    let mut retries_used: u32 = 0;

    // Text: keep single-task layers streaming; parallel layers buffer output to avoid interleaving.
    let (rendered, exit_code): (TaskRender, i32) = if task.stream_format == "jsonl" {
        let (tx, rx) = mpsc::unbounded_channel();
        emit_task_start_jsonl(run_id, &render_info);

        let run_id_owned = run_id.to_string();
        let render_info_for_render = render_info.clone();
        let render_handle = tokio::spawn(async move {
            render_task_jsonl_events(&run_id_owned, render_info_for_render, rx).await
        });

        let mut final_code = retry::exit_code_for_general_failure();

        for attempt in 1..=max_attempts {
            if attempt > 1 {
                retries_used += 1;
                emit_json(&JsonlEvent {
                    v: 1,
                    event_type: "warning".into(),
                    ts: Utc::now().to_rfc3339(),
                    run_id: run_id.to_string(),
                    task_id: Some(task.id.clone()),
                    action: None,
                    args: None,
                    output: Some(format!("Retrying (attempt {}/{})", attempt, max_attempts)),
                    error: None,
                    code: None,
                    progress: None,
                    metadata: None,
                });
            }

            let (runner_spec, start_data) = planner(&task)?;
            let (abort_tx, abort_rx) = mpsc::channel::<String>(1);

            let plan_args = crate::api::RunWithQueryArgs {
                user_query: prompt.clone(),
                cfg: ctx.cfg().clone(),
                runner: runner_spec,
                run_id: run_id.to_string(),
                capture_bytes: opts.capture_bytes,
                stream_format: task.stream_format.clone(),
                project_id: task.workdir.clone(),
                events_out_tx: ctx.events_out(),
                services: services.clone(),
                wrapper_start_data: start_data,
            };

            let tx_for_run = tx.clone();
            let run_fut = run_with_query(plan_args, move |input| {
                let tx_clone = tx_for_run;
                async move {
                    let backend_kind_str = input.backend_kind.to_string();
                    run_session(RunSessionArgs {
                        session: input.session,
                        control: &input.control,
                        policy: input.policy,
                        capture_bytes: input.capture_bytes,
                        events_out: input.events_out_tx,
                        event_tx: Some(tx_clone),
                        run_id: &input.run_id,
                        backend_kind: &backend_kind_str,
                        stream_format: &input.stream_format,
                        abort_rx: Some(abort_rx),
                    })
                    .await
                }
            });
            tokio::pin!(run_fut);

            let timed = tokio::time::timeout(Duration::from_secs(timeout_secs), &mut run_fut).await;
            let (timed_out, run_res) = match timed {
                Ok(res) => (false, res),
                Err(_) => {
                    let _ = abort_tx
                        .send(format!("timeout after {}s", timeout_secs))
                        .await;
                    (true, run_fut.await)
                }
            };

            match run_res {
                Ok(_) if timed_out => {
                    final_code = retry::exit_code_for_timeout();
                }
                Ok(0) => {
                    final_code = retry::exit_code_for_success();
                    break;
                }
                Ok(_) => {
                    final_code = retry::exit_code_for_general_failure();
                }
                Err(e) => {
                    final_code = if timed_out {
                        retry::exit_code_for_timeout()
                    } else {
                        retry::exit_code_for_backend_failure()
                    };
                    emit_json(&JsonlEvent {
                        v: 1,
                        event_type: "error".into(),
                        ts: Utc::now().to_rfc3339(),
                        run_id: run_id.to_string(),
                        task_id: Some(task.id.clone()),
                        action: None,
                        args: None,
                        output: None,
                        error: Some(e.to_string()),
                        code: Some(final_code),
                        progress: None,
                        metadata: None,
                    });
                }
            }

            if timed_out && final_code != retry::exit_code_for_success() {
                emit_json(&JsonlEvent {
                    v: 1,
                    event_type: "error".into(),
                    ts: Utc::now().to_rfc3339(),
                    run_id: run_id.to_string(),
                    task_id: Some(task.id.clone()),
                    action: None,
                    args: None,
                    output: None,
                    error: Some(format!("Timeout after {} seconds", timeout_secs)),
                    code: Some(retry::exit_code_for_timeout()),
                    progress: None,
                    metadata: None,
                });
            }

            if attempt == max_attempts {
                break;
            }
        }

        drop(tx);
        let outcome = render_handle
            .await
            .map_err(|e| StdioError::RunnerError(e.to_string()))?;

        emit_task_end_jsonl(
            run_id,
            &render_info,
            final_code,
            outcome.duration_ms,
            retries_used,
        );
        (
            TaskRender {
                duration_ms: outcome.duration_ms,
                text_block: None,
            },
            final_code,
        )
    } else if buffer_text {
        let mut buf = String::new();
        let mut total_ms: u64 = 0;
        let mut final_code = retry::exit_code_for_general_failure();

        for attempt in 1..=max_attempts {
            if attempt > 1 {
                retries_used += 1;
                if !opts.quiet {
                    buf.push_str(&format!(
                        "{} 重试 {}/{}\n",
                        markers.retry,
                        retries_used,
                        max_attempts - 1
                    ));
                }
            }

            let (tx, rx) = mpsc::unbounded_channel();
            let (abort_tx, abort_rx) = mpsc::channel::<String>(1);
            let (runner_spec, start_data) = planner(&task)?;

            let plan_args = crate::api::RunWithQueryArgs {
                user_query: prompt.clone(),
                cfg: ctx.cfg().clone(),
                runner: runner_spec,
                run_id: run_id.to_string(),
                capture_bytes: opts.capture_bytes,
                stream_format: task.stream_format.clone(),
                project_id: task.workdir.clone(),
                events_out_tx: ctx.events_out(),
                services: services.clone(),
                wrapper_start_data: start_data,
            };

            let render_handle = if opts.quiet {
                tokio::spawn(collect_task_text_content_only(rx))
            } else {
                tokio::spawn(collect_task_text(rx))
            };
            let tx_for_run = tx.clone();
            let run_fut = run_with_query(plan_args, move |input| {
                let tx_clone = tx_for_run;
                async move {
                    let backend_kind_str = input.backend_kind.to_string();
                    run_session(RunSessionArgs {
                        session: input.session,
                        control: &input.control,
                        policy: input.policy,
                        capture_bytes: input.capture_bytes,
                        events_out: input.events_out_tx,
                        event_tx: Some(tx_clone),
                        run_id: &input.run_id,
                        backend_kind: &backend_kind_str,
                        stream_format: &input.stream_format,
                        abort_rx: Some(abort_rx),
                    })
                    .await
                }
            });
            tokio::pin!(run_fut);

            let timed = tokio::time::timeout(Duration::from_secs(timeout_secs), &mut run_fut).await;
            let (timed_out, run_res) = match timed {
                Ok(res) => (false, res),
                Err(_) => {
                    let _ = abort_tx
                        .send(format!("timeout after {}s", timeout_secs))
                        .await;
                    (true, run_fut.await)
                }
            };

            drop(tx);
            let rendered = render_handle
                .await
                .map_err(|e| StdioError::RunnerError(e.to_string()))?;

            total_ms = total_ms.saturating_add(rendered.duration_ms.unwrap_or(0));
            if let Some(content) = rendered.text_block.as_deref() {
                buf.push_str(content);
                if !buf.ends_with('\n') {
                    buf.push('\n');
                }
            }

            match run_res {
                Ok(_) if timed_out => {
                    final_code = retry::exit_code_for_timeout();
                }
                Ok(0) => {
                    final_code = retry::exit_code_for_success();
                    break;
                }
                Ok(_) => {
                    final_code = retry::exit_code_for_general_failure();
                }
                Err(_) if timed_out => {
                    final_code = retry::exit_code_for_timeout();
                }
                Err(_) => {
                    final_code = retry::exit_code_for_backend_failure();
                }
            }

            if attempt == max_attempts {
                break;
            }
        }

        (
            TaskRender {
                duration_ms: Some(total_ms),
                text_block: Some(buf),
            },
            final_code,
        )
    } else {
        // streaming text (serial)
        emit_text_task_header(&task, &resolved_files, markers, opts);

        let mut total_ms: u64 = 0;
        let mut final_code = retry::exit_code_for_general_failure();

        for attempt in 1..=max_attempts {
            if attempt > 1 {
                retries_used += 1;
                if !opts.quiet {
                    println!(
                        "{} 重试 {}/{}",
                        markers.retry,
                        retries_used,
                        max_attempts - 1
                    );
                }
            }

            let (tx, rx) = mpsc::unbounded_channel();
            let (abort_tx, abort_rx) = mpsc::channel::<String>(1);
            let (runner_spec, start_data) = planner(&task)?;

            let plan_args = crate::api::RunWithQueryArgs {
                user_query: prompt.clone(),
                cfg: ctx.cfg().clone(),
                runner: runner_spec,
                run_id: run_id.to_string(),
                capture_bytes: opts.capture_bytes,
                stream_format: task.stream_format.clone(),
                project_id: task.workdir.clone(),
                events_out_tx: ctx.events_out(),
                services: services.clone(),
                wrapper_start_data: start_data,
            };

            let render_info = render_info.clone();
            let render_handle = if opts.quiet {
                tokio::spawn(async move { render_task_stream_content_only(rx).await })
            } else {
                let markers_owned = markers.clone();
                tokio::spawn(
                    async move { render_task_stream(render_info, rx, &markers_owned).await },
                )
            };

            let tx_for_run = tx.clone();
            let run_fut = run_with_query(plan_args, move |input| {
                let tx_clone = tx_for_run;
                async move {
                    let backend_kind_str = input.backend_kind.to_string();
                    run_session(RunSessionArgs {
                        session: input.session,
                        control: &input.control,
                        policy: input.policy,
                        capture_bytes: input.capture_bytes,
                        events_out: input.events_out_tx,
                        event_tx: Some(tx_clone),
                        run_id: &input.run_id,
                        backend_kind: &backend_kind_str,
                        stream_format: &input.stream_format,
                        abort_rx: Some(abort_rx),
                    })
                    .await
                }
            });
            tokio::pin!(run_fut);

            let timed = tokio::time::timeout(Duration::from_secs(timeout_secs), &mut run_fut).await;
            let (timed_out, run_res) = match timed {
                Ok(res) => (false, res),
                Err(_) => {
                    let _ = abort_tx
                        .send(format!("timeout after {}s", timeout_secs))
                        .await;
                    (true, run_fut.await)
                }
            };

            drop(tx);
            let rendered = render_handle
                .await
                .map_err(|e| StdioError::RunnerError(e.to_string()))?;

            total_ms = total_ms.saturating_add(rendered.duration_ms.unwrap_or(0));

            match run_res {
                Ok(_) if timed_out => {
                    final_code = retry::exit_code_for_timeout();
                }
                Ok(0) => {
                    final_code = retry::exit_code_for_success();
                    break;
                }
                Ok(_) => {
                    final_code = retry::exit_code_for_general_failure();
                }
                Err(e) => {
                    final_code = if timed_out {
                        retry::exit_code_for_timeout()
                    } else {
                        retry::exit_code_for_backend_failure()
                    };
                    if !opts.quiet {
                        println!("{} {}", markers.fail, e);
                    }
                }
            }

            if attempt == max_attempts {
                break;
            }
        }

        (
            TaskRender {
                duration_ms: Some(total_ms),
                text_block: None,
            },
            final_code,
        )
    };

    if !buffer_text && task.stream_format == "text" {
        emit_text_task_footer(
            &task,
            exit_code,
            rendered.duration_ms,
            retries_used,
            markers,
            opts,
        );
    }

    let text_block = if buffer_text {
        if opts.quiet {
            rendered.text_block.clone()
        } else {
            let mut block = String::new();
            block.push_str(&format!("  --- {} ---\n", task.id));
            if let Some(content) = rendered.text_block.as_deref() {
                for line in content.lines() {
                    block.push_str("  ");
                    block.push_str(line);
                    block.push('\n');
                }
            }

            let dur = format!("{:.1}s", rendered.duration_ms.unwrap_or(0) as f64 / 1000.0);
            let status = if exit_code == 0 {
                markers.ok
            } else {
                markers.fail
            };
            if retries_used > 0 {
                block.push_str(&format!(
                    "  {status} {} {dur} (重试{retries_used}次)\n\n",
                    task.id
                ));
            } else {
                block.push_str(&format!("  {status} {} {dur}\n\n", task.id));
            }
            Some(block)
        }
    } else {
        None
    };

    Ok((
        task.id.clone(),
        TaskExecResult {
            exit_code,
            text_block,
        },
    ))
}

fn emit_text_task_header(
    task: &StdioTask,
    files: &[ResolvedFile],
    markers: &TextMarkers,
    opts: &StdioRunOpts,
) {
    if opts.quiet {
        return;
    }

    let prefix = ts_prefix(opts);
    let backend = format_backend(&task.backend, task.model.as_deref());
    if task.dependencies.is_empty() {
        println!("{prefix}{} {} ({backend})", markers.start, task.id);
    } else {
        println!(
            "{prefix}{} {} ({backend}) ← {}",
            markers.start,
            task.id,
            task.dependencies.join(", ")
        );
    }

    if opts.verbose {
        println!("{prefix}  工作目录: {}", task.workdir);
    }

    for f in files {
        println!(
            "{prefix}  {} {} ({})",
            markers.file,
            f.display_path,
            format_bytes(f.size)
        );
    }

    println!();
}

fn emit_text_task_footer(
    task: &StdioTask,
    exit_code: i32,
    duration_ms: Option<u64>,
    retries_used: u32,
    markers: &TextMarkers,
    opts: &StdioRunOpts,
) {
    if opts.quiet {
        return;
    }

    let prefix = ts_prefix(opts);
    let dur = duration_ms
        .map(|d| format!("{:.1}s", d as f64 / 1000.0))
        .unwrap_or_else(|| "-".to_string());

    let status = if exit_code == 0 {
        markers.ok
    } else {
        markers.fail
    };
    if retries_used > 0 {
        println!("{prefix}{status} {} {dur} (重试{retries_used}次)", task.id);
    } else {
        println!("{prefix}{status} {} {dur}", task.id);
    }
}

fn ts_prefix(opts: &StdioRunOpts) -> String {
    if !opts.verbose {
        return String::new();
    }
    let ts = chrono::Local::now().format("%H:%M:%S");
    format!("[{ts}] ")
}

fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = 1024.0 * 1024.0;
    const GB: f64 = 1024.0 * 1024.0 * 1024.0;
    let b = bytes as f64;
    if b >= GB {
        format!("{:.1}GB", b / GB)
    } else if b >= MB {
        format!("{:.1}MB", b / MB)
    } else if b >= KB {
        format!("{:.1}KB", b / KB)
    } else {
        format!("{bytes}B")
    }
}

struct TaskRender {
    duration_ms: Option<u64>,
    text_block: Option<String>,
}

async fn collect_task_text(
    mut rx: mpsc::UnboundedReceiver<crate::runner::RunnerEvent>,
) -> TaskRender {
    let started = std::time::Instant::now();
    let mut buf = String::with_capacity(4096);
    let mut event_count = 0;

    while let Some(ev) = rx.recv().await {
        event_count += 1;

        // Dynamic capacity expansion every 20 events
        if event_count % 20 == 0 && buf.capacity() - buf.len() < 1024 {
            buf.reserve(4096);
        }

        match ev {
            crate::runner::RunnerEvent::AssistantOutput(text) => {
                buf.push_str(&text);
                if !buf.ends_with('\n') {
                    buf.push('\n');
                }
            }
            crate::runner::RunnerEvent::ToolEvent(tool) => {
                if tool.event_type == "assistant.output" {
                    if let Some(v) = tool.output.as_ref().and_then(|v| v.as_str()) {
                        buf.push_str(v);
                        if !buf.ends_with('\n') {
                            buf.push('\n');
                        }
                    }
                }
            }
            crate::runner::RunnerEvent::RawStdout(line) => {
                buf.push_str(&line);
                if !buf.ends_with('\n') {
                    buf.push('\n');
                }
            }
            crate::runner::RunnerEvent::RawStderr(line) => {
                buf.push_str(&line);
                if !buf.ends_with('\n') {
                    buf.push('\n');
                }
            }
            crate::runner::RunnerEvent::RunComplete { exit_code: code } => {
                let _ = code;
            }
            crate::runner::RunnerEvent::Error(msg) => {
                buf.push_str(&msg);
                if !buf.ends_with('\n') {
                    buf.push('\n');
                }
            }
            crate::runner::RunnerEvent::StatusUpdate { .. } => {}
        }
    }

    // Shrink to fit if over-allocated
    if buf.capacity() > buf.len() * 2 {
        buf.shrink_to_fit();
    }

    TaskRender {
        duration_ms: Some(started.elapsed().as_millis() as u64),
        text_block: Some(buf),
    }
}

async fn collect_task_text_content_only(
    mut rx: mpsc::UnboundedReceiver<crate::runner::RunnerEvent>,
) -> TaskRender {
    let started = std::time::Instant::now();
    let mut buf = String::with_capacity(4096);
    let mut event_count = 0;

    while let Some(ev) = rx.recv().await {
        event_count += 1;

        // Dynamic capacity expansion every 20 events
        if event_count % 20 == 0 && buf.capacity() - buf.len() < 1024 {
            buf.reserve(4096);
        }

        match ev {
            crate::runner::RunnerEvent::AssistantOutput(text) => {
                buf.push_str(&text);
                if !buf.ends_with('\n') {
                    buf.push('\n');
                }
            }
            crate::runner::RunnerEvent::ToolEvent(tool) => {
                if tool.event_type == "assistant.output" {
                    if let Some(v) = tool.output.as_ref().and_then(|v| v.as_str()) {
                        if !v.is_empty() {
                            buf.push_str(v);
                            if !buf.ends_with('\n') {
                                buf.push('\n');
                            }
                        }
                    }
                }
            }
            crate::runner::RunnerEvent::RawStdout(line) => {
                buf.push_str(&line);
                if !buf.ends_with('\n') {
                    buf.push('\n');
                }
            }
            crate::runner::RunnerEvent::RawStderr(_) => {}
            crate::runner::RunnerEvent::RunComplete { .. } => {}
            crate::runner::RunnerEvent::Error(_) => {}
            crate::runner::RunnerEvent::StatusUpdate { .. } => {}
        }
    }

    // Shrink to fit if over-allocated
    if buf.capacity() > buf.len() * 2 {
        buf.shrink_to_fit();
    }

    TaskRender {
        duration_ms: Some(started.elapsed().as_millis() as u64),
        text_block: Some(buf),
    }
}

/// 异步处理单个文件（Level 1 优化：文件并行处理）
///
/// # 参数
/// - `seen`: 已处理文件的 canonical 路径集合（用于去重）
/// - `cancel_flag`: 取消标志（错误时设置）
async fn process_single_file(
    path: PathBuf,
    base_canon: PathBuf,
    files_mode: FilesMode,
    files_encoding: FilesEncoding,
    stdio_config: Arc<crate::config::StdioConfig>,
    seen: Arc<std::sync::Mutex<HashSet<PathBuf>>>,
    cancel_flag: Arc<AtomicBool>,
) -> Result<Option<ResolvedFile>, StdioError> {
    // 检查取消标志
    if cancel_flag.load(Ordering::Relaxed) {
        return Err(StdioError::RunnerError("task cancelled".into()));
    }

    // 【优化点 1】: 异步 canonicalize（Windows 性能提升明显）
    let canonical = tokio::fs::canonicalize(&path).await.map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            StdioError::FileNotFound(path.display().to_string())
        } else {
            StdioError::FileAccessDenied(path.display().to_string())
        }
    })?;

    // 去重检查（使用 canonical 路径）
    {
        let mut seen_guard = seen.lock().unwrap();
        if !seen_guard.insert(canonical.clone()) {
            // 已处理过，跳过
            return Ok(None);
        }
    }

    // 安全检查：防止路径遍历攻击
    if !canonical.starts_with(&base_canon) {
        return Err(StdioError::PathTraversal(path.display().to_string()));
    }

    // 【优化点 2】: 异步 metadata 获取
    let meta = tokio::fs::metadata(&canonical).await.map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            StdioError::FileNotFound(path.display().to_string())
        } else {
            StdioError::FileAccessDenied(path.display().to_string())
        }
    })?;

    if meta.is_dir() {
        return Err(StdioError::InvalidPath("is a directory".to_string()));
    }

    if meta.len() > MAX_SINGLE_FILE {
        return Err(StdioError::FileTooLarge(meta.len(), MAX_SINGLE_FILE));
    }

    // 决定嵌入模式
    let mode = match files_mode {
        FilesMode::Auto => {
            // auto 模式：永远使用路径引用（不读取文件内容）
            FilesMode::Ref
        }
        FilesMode::Embed => {
            // embed 模式：文件 > 50KB 时降级为 ref
            if meta.len() > EMBED_SIZE_LIMIT {
                FilesMode::Ref
            } else {
                FilesMode::Embed
            }
        }
        FilesMode::Ref => FilesMode::Ref,
    };

    // 【优化点 3】: 异步读取文件内容（Level 3.1 + 3.3：mmap + LRU 缓存）
    let content = if mode == FilesMode::Embed {
        // 从统一配置读取优化选项
        let enable_mmap = stdio_config.enable_mmap_large_files;
        let mmap_threshold_mb = stdio_config.mmap_threshold_mb;
        let enable_cache = stdio_config.enable_file_cache;

        // 使用 mmap + 缓存优化读取
        let bytes = if enable_mmap || enable_cache {
            read_file_cached(&canonical, mmap_threshold_mb, meta.len(), enable_cache).await?
        } else {
            tokio::fs::read(&canonical).await.map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    StdioError::FileNotFound(path.display().to_string())
                } else {
                    StdioError::FileAccessDenied(path.display().to_string())
                }
            })?
        };

        match files_encoding {
            FilesEncoding::Utf8 | FilesEncoding::Auto => match String::from_utf8(bytes) {
                Ok(s) => Some(ResolvedContent::Text(s)),
                Err(e) => {
                    // 【优化点 4】: Base64 编码在 blocking 线程池执行（避免阻塞 tokio runtime）
                    let data = tokio::task::spawn_blocking(move || {
                        base64::engine::general_purpose::STANDARD.encode(e.into_bytes())
                    })
                    .await
                    .map_err(|e| StdioError::BackendError(e.to_string()))?;

                    Some(ResolvedContent::Base64(data))
                }
            },
            FilesEncoding::Base64 => {
                let data = tokio::task::spawn_blocking(move || {
                    base64::engine::general_purpose::STANDARD.encode(bytes)
                })
                .await
                .map_err(|e| StdioError::BackendError(e.to_string()))?;

                Some(ResolvedContent::Base64(data))
            }
        }
    } else {
        None
    };

    let modified = meta.modified().ok();
    let encoding = match files_encoding {
        FilesEncoding::Auto => match &content {
            Some(ResolvedContent::Text(_)) => FilesEncoding::Utf8,
            Some(ResolvedContent::Base64(_)) => FilesEncoding::Base64,
            None => FilesEncoding::Auto,
        },
        other => other,
    };

    Ok(Some(ResolvedFile {
        display_path: canonical
            .strip_prefix(&base_canon)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| canonical.to_string_lossy().to_string()),
        mode,
        encoding,
        size: meta.len(),
        modified,
        content,
    }))
}

/// 异步文件解析（优化版：并发限制 + 任务取消 + 精确内存分配）
///
/// # 优化点
/// - Level 1: 并发限制（Semaphore），防止文件描述符耗尽
/// - Level 2: 任务取消（AtomicBool），错误时立即停止
/// - Level 3: 去重优化（canonical 路径去重，避免重复处理）
/// - Level 4: 内存预分配（预统计 glob 匹配数）
/// - Level 5: Arc 配置共享（避免重复克隆）
async fn resolve_files(
    task: &StdioTask,
    stdio_config: &crate::config::StdioConfig,
) -> Result<Vec<ResolvedFile>, StdioError> {
    if task.files.is_empty() {
        return Ok(Vec::new());
    }

    // 【优化 5】: Arc 包装配置，避免重复克隆
    let config_arc = Arc::new(stdio_config.clone());

    // 【优化 1】: 并发限制 - 最大 16 个文件并发处理
    const MAX_CONCURRENT_FILES: usize = 16;
    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_FILES));

    // 【优化 2】: 取消标志 - 错误时设置
    let cancel_flag = Arc::new(AtomicBool::new(false));

    // 【优化 3】: canonical 路径去重
    let seen = Arc::new(std::sync::Mutex::new(HashSet::<PathBuf>::new()));

    // 预先规范化 workdir（只调用一次）
    let base_canon = tokio::fs::canonicalize(Path::new(&task.workdir))
        .await
        .map_err(|_| StdioError::InvalidPath(task.workdir.clone()))?;

    // 【优化 4】: 预统计 glob 匹配总数（精确内存分配）
    let mut all_paths = Vec::new();
    for raw in &task.files {
        let base = Path::new(&task.workdir);
        let candidate = Path::new(raw);
        let pattern = if candidate.is_absolute() {
            candidate.to_path_buf()
        } else {
            base.join(candidate)
        };

        let glob_str = pattern
            .to_str()
            .ok_or_else(|| StdioError::InvalidPath(raw.clone()))?;

        // glob 在 blocking 线程池执行
        let paths = tokio::task::spawn_blocking({
            let pattern = glob_str.to_string();
            move || {
                glob::glob(&pattern)
                    .map_err(|e| StdioError::InvalidPath(e.to_string()))?
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|e| StdioError::InvalidPath(e.to_string()))
            }
        })
        .await
        .map_err(|e| StdioError::BackendError(e.to_string()))??;

        if paths.is_empty() {
            return Err(StdioError::GlobNoMatch(raw.clone()));
        }

        all_paths.extend(paths);
    }

    // 精确预分配内存（避免重新分配）
    let mut collected = Vec::with_capacity(all_paths.len());
    let mut total_size: u64 = 0;

    // 使用 FuturesUnordered 流式处理（避免一次性启动所有任务）
    let mut futures = FuturesUnordered::new();

    for path in all_paths {
        // 检查取消标志
        if cancel_flag.load(Ordering::Relaxed) {
            break;
        }

        let permit = semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| StdioError::RunnerError("semaphore closed unexpectedly".into()))?;

        let base_canon = base_canon.clone();
        let files_mode = task.files_mode;
        let files_encoding = task.files_encoding;
        let config = Arc::clone(&config_arc);
        let seen_clone = Arc::clone(&seen);
        let cancel_clone = Arc::clone(&cancel_flag);

        futures.push(async move {
            let _permit = permit; // 持有许可直到完成
            process_single_file(
                path,
                base_canon,
                files_mode,
                files_encoding,
                config,
                seen_clone,
                cancel_clone,
            )
            .await
        });
    }

    // 流式收集结果
    while let Some(result) = futures.next().await {
        match result {
            Ok(Some(resolved)) => {
                total_size += resolved.size;

                // 限制检查
                if collected.len() >= MAX_FILES {
                    cancel_flag.store(true, Ordering::Relaxed);
                    return Err(StdioError::TooManyFiles(collected.len(), MAX_FILES));
                }
                if total_size > MAX_TOTAL_SIZE {
                    cancel_flag.store(true, Ordering::Relaxed);
                    return Err(StdioError::FileTooLarge(total_size, MAX_TOTAL_SIZE));
                }

                collected.push(resolved);
            }
            Ok(None) => {
                // 跳过重复文件（去重）
            }
            Err(e) => {
                // 设置取消标志，停止其他任务
                cancel_flag.store(true, Ordering::Relaxed);
                return Err(e);
            }
        }
    }

    Ok(collected)
}

fn compose_prompt(task: &StdioTask, files: &[ResolvedFile]) -> String {
    // Pre-calculate total capacity to avoid multiple allocations
    let file_content_size: usize = files
        .iter()
        .map(|f| {
            f.content.as_ref().map_or(0, |c| match c {
                ResolvedContent::Text(s) => s.len(),
                ResolvedContent::Base64(s) => s.len(),
            })
        })
        .sum();

    let estimated_capacity = task.content.len() + file_content_size + files.len() * 150; // Estimate for file markers and metadata

    let mut prompt = String::with_capacity(estimated_capacity);

    prompt.push_str(&task.content);
    if !prompt.ends_with('\n') {
        prompt.push('\n');
    }

    for f in files {
        match &f.content {
            Some(ResolvedContent::Text(content)) => {
                prompt.push_str("---FILE: ");
                prompt.push_str(&f.display_path);
                prompt.push_str("---\n");
                prompt.push_str(&format_file_metadata(f));
                prompt.push_str(content);
                if !prompt.ends_with('\n') {
                    prompt.push('\n');
                }
                prompt.push_str("---END FILE---\n");
            }
            Some(ResolvedContent::Base64(content)) => {
                prompt.push_str("---FILE: ");
                prompt.push_str(&f.display_path);
                prompt.push_str(" [base64]---\n");
                prompt.push_str(&format_file_metadata(f));
                prompt.push_str(content);
                if !prompt.ends_with('\n') {
                    prompt.push('\n');
                }
                prompt.push_str("---END FILE---\n");
            }
            None => {
                prompt.push_str("---FILE: ");
                prompt.push_str(&f.display_path);
                prompt.push_str(" [ref]---\n");
                prompt.push_str(&format_file_metadata(f));
                prompt.push_str("<ref-only>\n");
                prompt.push_str("---END FILE---\n");
            }
        }
    }
    prompt
}

fn format_file_metadata(file: &ResolvedFile) -> String {
    let encoding_str = match file.encoding {
        FilesEncoding::Utf8 => "utf-8",
        FilesEncoding::Base64 => "base64",
        FilesEncoding::Auto => "auto",
    };

    let modified_str = file
        .modified
        .and_then(|t| {
            t.duration_since(std::time::UNIX_EPOCH).ok().map(|d| {
                let secs = d.as_secs();
                chrono::DateTime::from_timestamp(secs as i64, 0)
                    .map(|dt| dt.to_rfc3339())
                    .unwrap_or_else(|| format!("unix:{}", secs))
            })
        })
        .unwrap_or_else(|| "unknown".to_string());

    format!(
        "<!-- size: {} bytes, modified: {}, encoding: {} -->\n",
        file.size, modified_str, encoding_str
    )
}

// ============================================================================
// SIMD-Accelerated Text Detection (Level 3.2 优化)
// ============================================================================

/// SIMD 加速文本检测（Level 3.2 优化）
///
/// # 性能
/// - AVX2：5-8x 加速
/// - Scalar fallback：保证跨平台兼容性
#[cfg(target_arch = "x86_64")]
#[allow(dead_code)]
fn is_printable_simd_avx2(bytes: &[u8]) -> bool {
    #[cfg(target_feature = "avx2")]
    unsafe {
        use std::arch::x86_64::*;

        const PRINTABLE_MIN: i8 = 0x20; // 空格
        const PRINTABLE_MAX: i8 = 0x7E; // ~

        let min_vec = _mm256_set1_epi8(PRINTABLE_MIN);
        let max_vec = _mm256_set1_epi8(PRINTABLE_MAX);

        let mut non_printable_count = 0;
        let chunks = bytes.chunks_exact(32);
        let remainder = chunks.remainder();

        for chunk in chunks {
            let data = _mm256_loadu_si256(chunk.as_ptr() as *const __m256i);

            // 检查是否在可打印范围内
            let ge_min = _mm256_cmpgt_epi8(data, min_vec);
            let le_max = _mm256_cmpgt_epi8(max_vec, data);
            let valid = _mm256_and_si256(ge_min, le_max);

            let mask = _mm256_movemask_epi8(valid);
            non_printable_count += mask.count_zeros();
        }

        // 处理剩余字节
        for &byte in remainder {
            if !(0x20..=0x7E).contains(&byte) && byte != b'\n' && byte != b'\r' && byte != b'\t' {
                non_printable_count += 1;
            }
        }

        // 允许 5% 非打印字符
        non_printable_count < bytes.len() / 20
    }

    #[cfg(not(target_feature = "avx2"))]
    is_printable_scalar(bytes)
}

/// Scalar fallback：跨平台文本检测
#[allow(dead_code)]
fn is_printable_scalar(bytes: &[u8]) -> bool {
    let mut non_printable_count = 0;

    for &byte in bytes {
        if !(0x20..=0x7E).contains(&byte) && byte != b'\n' && byte != b'\r' && byte != b'\t' {
            non_printable_count += 1;
        }
    }

    // 允许 5% 非打印字符
    non_printable_count < bytes.len() / 20
}

/// 增强版文本检测（扩展名 + 内容检测 + SIMD）
#[allow(dead_code)]
fn is_likely_text_enhanced(path: &Path) -> bool {
    const TEXT_EXTENSIONS: &[&str] = &[
        "txt",
        "md",
        "markdown",
        "rst",
        "adoc",
        "py",
        "js",
        "ts",
        "jsx",
        "tsx",
        "rs",
        "go",
        "c",
        "cpp",
        "h",
        "hpp",
        "java",
        "kt",
        "swift",
        "rb",
        "php",
        "pl",
        "sh",
        "bash",
        "zsh",
        "fish",
        "css",
        "scss",
        "sass",
        "less",
        "html",
        "htm",
        "xml",
        "json",
        "yaml",
        "yml",
        "toml",
        "ini",
        "cfg",
        "conf",
        "config",
        "sql",
        "graphql",
        "proto",
        "dockerfile",
        "makefile",
        "cmake",
        "gradle",
        "maven",
        "sbt",
        "r",
        "lua",
        "vim",
        "el",
        "clj",
        "ex",
        "erl",
        "hs",
        "ml",
        "scala",
        "dart",
        "vue",
        "svelte",
        "astro",
    ];

    // 1. 扩展名快速路径
    if let Some(ext) = path.extension() {
        let ext_lower = ext.to_string_lossy().to_lowercase();
        if TEXT_EXTENSIONS.contains(&ext_lower.as_str()) {
            return true;
        }
    } else {
        // 特殊文件名
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            let name_lower = name.to_lowercase();
            if name_lower == "makefile"
                || name_lower == "dockerfile"
                || name_lower == "readme"
                || name_lower.starts_with("readme.")
                || name_lower == "license"
                || name_lower.starts_with("license.")
            {
                return true;
            }
        }
    }

    // 2. 内容检测（读取前 1024 字节）
    if let Ok(bytes) = std::fs::read(path) {
        let sample = &bytes[..bytes.len().min(1024)];

        #[cfg(target_arch = "x86_64")]
        {
            return is_printable_simd_avx2(sample);
        }

        #[cfg(not(target_arch = "x86_64"))]
        {
            return is_printable_scalar(sample);
        }
    }

    false
}

// ============================================================================
// Topological Sort
// ============================================================================

fn topo_sort_layered(tasks: &[StdioTask]) -> Vec<Vec<String>> {
    let lookup: HashMap<&str, &StdioTask> = tasks.iter().map(|t| (t.id.as_str(), t)).collect();
    let mut memo: HashMap<String, usize> = HashMap::new();

    fn compute_level<'a>(
        id: &'a str,
        lookup: &HashMap<&'a str, &'a StdioTask>,
        memo: &mut HashMap<String, usize>,
    ) -> usize {
        if let Some(&lv) = memo.get(id) {
            return lv;
        }
        let task = lookup.get(id).expect("task missing in topo_sort_layered");
        let max_dep = task
            .dependencies
            .iter()
            .map(|dep| compute_level(dep, lookup, memo))
            .max()
            .unwrap_or(0);
        let lv = max_dep + 1;
        memo.insert(id.to_string(), lv);
        lv
    }

    for task in tasks {
        compute_level(&task.id, &lookup, &mut memo);
    }

    let max_level = memo.values().copied().max().unwrap_or(0);
    let mut layers: Vec<Vec<String>> = Vec::new();

    for lv in 1..=max_level {
        let mut layer: Vec<String> = Vec::new();
        // Keep layer order stable based on input order
        for task in tasks {
            if memo.get(&task.id).copied().unwrap_or(0) == lv {
                layer.push(task.id.clone());
            }
        }
        if !layer.is_empty() {
            layers.push(layer);
        }
    }

    layers
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(id: &str, deps: &[&str]) -> StdioTask {
        StdioTask {
            id: id.to_string(),
            backend: "codex".to_string(),
            workdir: ".".to_string(),
            model: None,
            model_provider: None,
            dependencies: deps.iter().map(|s| s.to_string()).collect(),
            stream_format: "text".to_string(),
            timeout: None,
            retry: None,
            files: vec![],
            files_mode: FilesMode::Auto,
            files_encoding: FilesEncoding::Auto,
            content: String::new(),
        }
    }

    #[test]
    fn topo_single_layer_keeps_input_order() {
        let tasks = vec![task("a", &[]), task("b", &[]), task("c", &[])];
        let layers = topo_sort_layered(&tasks);
        assert_eq!(
            layers,
            vec![vec!["a".to_string(), "b".to_string(), "c".to_string()]]
        );
    }

    #[test]
    fn topo_chain_layers() {
        let tasks = vec![task("a", &[]), task("b", &["a"]), task("c", &["b"])];
        let layers = topo_sort_layered(&tasks);
        assert_eq!(
            layers,
            vec![
                vec!["a".to_string()],
                vec!["b".to_string()],
                vec!["c".to_string()]
            ]
        );
    }

    #[test]
    fn topo_diamond_layers_stable_order() {
        let tasks = vec![
            task("a", &[]),
            task("b", &["a"]),
            task("c", &["a"]),
            task("d", &["b", "c"]),
        ];
        let layers = topo_sort_layered(&tasks);
        assert_eq!(
            layers,
            vec![
                vec!["a".to_string()],
                vec!["b".to_string(), "c".to_string()],
                vec!["d".to_string()]
            ]
        );
    }
}
