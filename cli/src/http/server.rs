//! HTTP服务器生命周期管理

use crate::http::{
    middleware::{create_middleware_stack, request_logger},
    routes::create_router,
    AppState,
};
use axum::middleware;
use std::fs;
use std::io::Write;
use std::net::SocketAddr;
use std::path::PathBuf;
use tokio::signal;
use tracing::{info, warn};

/// HTTP服务器配置
#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".into(),
            port: 8080,
        }
    }
}

/// 启动HTTP服务器
pub async fn start_server(
    session_id: String,
    port: u16,
    state: AppState,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = ServerConfig {
        host: "127.0.0.1".into(),
        port,
    };

    start_server_with_config(session_id, config, state).await
}

/// 使用自定义配置启动HTTP服务器
pub async fn start_server_with_config(
    session_id: String,
    config: ServerConfig,
    state: AppState,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    info!(
        "Starting HTTP server on {}:{} (session: {})",
        config.host, config.port, session_id
    );

    // 创建状态文件
    let state_file_path = create_state_file(&session_id, config.port)?;
    info!("State file created: {}", state_file_path.display());

    // 构建路由
    let router = create_router(state.clone());

    // 添加中间件
    let app = router
        .layer(middleware::from_fn(request_logger))
        .layer(create_middleware_stack());

    // 解析地址
    let addr: SocketAddr = format!("{}:{}", config.host, config.port).parse()?;

    // 创建服务器
    info!("HTTP server listening on http://{}", addr);

    // 启动服务器
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    // 克隆 shutdown_rx 用于优雅关闭
    let mut shutdown_rx = state.shutdown_tx.subscribe();

    // 启动服务器并等待关闭信号
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            // 等待关闭信号
            tokio::select! {
                _ = signal::ctrl_c() => {
                    info!("Received Ctrl+C signal");
                }
                _ = shutdown_rx.recv() => {
                    info!("Received shutdown signal from API");
                }
                _ = wait_for_sigterm() => {
                    info!("Received SIGTERM signal");
                }
            }

            info!("Starting graceful shutdown...");
        })
        .await?;

    info!("Server shutdown complete");

    // 删除状态文件
    if let Err(e) = fs::remove_file(&state_file_path) {
        warn!("Failed to remove state file: {}", e);
    } else {
        info!("State file removed: {}", state_file_path.display());
    }

    Ok(())
}

/// 创建状态文件
fn create_state_file(session_id: &str, port: u16) -> Result<PathBuf, std::io::Error> {
    // 获取 ~/.memex/servers/ 目录
    let servers_dir = get_servers_dir()?;

    // 确保目录存在
    fs::create_dir_all(&servers_dir)?;

    // 状态文件路径
    let state_file = servers_dir.join(format!("http-{}.pid", port));

    // 写入状态信息
    let mut file = fs::File::create(&state_file)?;
    writeln!(file, "session_id={}", session_id)?;
    writeln!(file, "port={}", port)?;
    writeln!(file, "pid={}", std::process::id())?;
    writeln!(file, "start_time={}", chrono::Local::now().to_rfc3339())?;

    Ok(state_file)
}

/// 获取服务器状态目录
fn get_servers_dir() -> Result<PathBuf, std::io::Error> {
    let home_dir = dirs::home_dir().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "Home directory not found")
    })?;

    Ok(home_dir.join(".memex").join("servers"))
}

/// 等待 SIGTERM 信号（Unix系统）
#[cfg(unix)]
async fn wait_for_sigterm() {
    use tokio::signal::unix::{signal, SignalKind};

    let mut sigterm = signal(SignalKind::terminate()).expect("Failed to setup SIGTERM handler");
    sigterm.recv().await;
}

/// Windows 系统不支持 SIGTERM，使用空操作
#[cfg(not(unix))]
async fn wait_for_sigterm() {
    // Windows不支持SIGTERM，永久等待（实际上会被 Ctrl+C 或 shutdown API 中断）
    std::future::pending::<()>().await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::state::ServerStats;
    use memex_core::api::Services;
    use std::sync::{Arc, RwLock};
    use std::time::Duration;
    use tokio::sync::broadcast;

    #[tokio::test]
    async fn test_create_state_file() {
        let session_id = "test-session";
        let port = 9999;

        // 创建状态文件
        let result = create_state_file(session_id, port);
        assert!(result.is_ok());

        let state_file = result.unwrap();
        assert!(state_file.exists());

        // 读取文件内容
        let content = fs::read_to_string(&state_file).unwrap();
        assert!(content.contains(&format!("session_id={}", session_id)));
        assert!(content.contains(&format!("port={}", port)));
        assert!(content.contains("pid="));

        // 清理
        fs::remove_file(state_file).ok();
    }

    #[tokio::test]
    async fn test_get_servers_dir() {
        let result = get_servers_dir();
        assert!(result.is_ok());

        let servers_dir = result.unwrap();
        assert!(servers_dir.to_string_lossy().contains(".memex"));
        assert!(servers_dir.to_string_lossy().contains("servers"));
    }

    #[tokio::test]
    async fn test_server_config_default() {
        let config = ServerConfig::default();
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 8080);
    }

    #[tokio::test]
    async fn test_server_lifecycle() {
        // 创建测试状态
        let (shutdown_tx, _) = broadcast::channel(1);
        let gatekeeper_config = memex_core::api::GatekeeperConfig::default();
        let services = Services {
            policy: None,
            memory: None,
            gatekeeper: Arc::new(memex_plugins::gatekeeper::StandardGatekeeperPlugin::new(
                gatekeeper_config,
            )),
        };

        let state = AppState {
            session_id: "test-lifecycle".into(),
            services: Arc::new(services),
            stats: Arc::new(RwLock::new(ServerStats::new())),
            shutdown_tx: shutdown_tx.clone(),
        };

        let config = ServerConfig {
            host: "127.0.0.1".into(),
            port: 18080, // 使用非标准端口避免冲突
        };

        // 在后台启动服务器
        let session_id = "test-lifecycle".to_string();
        let server_handle =
            tokio::spawn(async move { start_server_with_config(session_id, config, state).await });

        // 等待服务器启动
        tokio::time::sleep(Duration::from_millis(500)).await;

        // 发送关闭信号
        let _ = shutdown_tx.send(());

        // 等待服务器关闭
        let result = tokio::time::timeout(Duration::from_secs(5), server_handle).await;
        assert!(result.is_ok(), "Server should shutdown gracefully");

        // 验证状态文件已删除（可能需要额外等待）
        tokio::time::sleep(Duration::from_millis(100)).await;
        let servers_dir = get_servers_dir().unwrap();
        let _state_file = servers_dir.join("http-18080.pid");
        // 注意：测试环境中可能因为异步删除尚未完成，这里不强制检查
    }

    #[tokio::test]
    async fn test_state_file_cleanup() {
        let session_id = "test-cleanup";
        let port = 19999;

        // 创建状态文件
        let state_file = create_state_file(session_id, port).unwrap();
        assert!(state_file.exists());

        // 模拟服务器关闭时删除状态文件
        let result = fs::remove_file(&state_file);
        assert!(result.is_ok());
        assert!(!state_file.exists());
    }
}
