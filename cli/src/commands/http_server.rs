//! HTTP服务器命令处理器

use crate::commands::cli::HttpServerArgs;
use crate::http::{server, AppState};
use memex_core::api::{AppContext, CliError};
use std::fs;
use std::path::PathBuf;
use tokio::sync::broadcast;
use uuid::Uuid;

/// 获取服务器状态文件目录
fn get_servers_dir() -> Result<PathBuf, CliError> {
    let home = dirs::home_dir()
        .ok_or_else(|| CliError::Command("Cannot find home directory".to_string()))?;
    let servers_dir = home.join(".memex").join("servers");
    fs::create_dir_all(&servers_dir)
        .map_err(|e| CliError::Command(format!("Failed to create servers directory: {e}")))?;
    Ok(servers_dir)
}

/// 写入服务器状态文件
fn write_state_file(session_id: &str, port: u16, host: &str) -> Result<(), CliError> {
    let servers_dir = get_servers_dir()?;
    let state_file = servers_dir.join("memex.state");

    let state = serde_json::json!({
        "session_id": session_id,
        "port": port,
        "pid": std::process::id(),
        "url": format!("http://{}:{}", host, port),
        "started_at": chrono::Local::now().to_rfc3339()
    });

    fs::write(&state_file, serde_json::to_string_pretty(&state).unwrap())
        .map_err(|e| CliError::Command(format!("Failed to write state file: {e}")))?;

    tracing::info!("State file written to: {}", state_file.display());
    Ok(())
}

/// 处理 http-server 命令
pub async fn handle_http_server(args: HttpServerArgs, ctx: &AppContext) -> Result<(), CliError> {
    // 使用用户提供的 session_id 或生成新的
    let session_id = args
        .session_id
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    // 合并配置：CLI 参数优先，配置文件作为默认值
    let config = &ctx.cfg().http_server;

    // 如果 CLI 参数与默认值相同，则使用配置文件中的值
    let port = if args.port == 8080 {
        config.port
    } else {
        args.port
    };

    let host = if args.host == "127.0.0.1" {
        config.host.clone()
    } else {
        args.host.clone()
    };

    // 构建 Services
    let services = ctx.build_services(ctx.cfg()).map_err(CliError::Runner)?;

    // 创建 shutdown channel
    let (shutdown_tx, _) = broadcast::channel(1);

    // 创建 AppState（传入完整配置）
    let state = AppState::new(session_id.clone(), services, ctx.cfg().clone(), shutdown_tx);

    // 写入状态文件（在服务器启动前）
    write_state_file(&session_id, port, &host)?;

    // 启动服务器
    tracing::info!(
        "Starting HTTP server on {}:{} (session: {})",
        host,
        port,
        session_id
    );

    server::start_server(session_id, host, port, state)
        .await
        .map_err(|e| CliError::Command(e.to_string()))?;

    Ok(())
}
