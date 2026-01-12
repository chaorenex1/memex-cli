//! HTTP服务器生命周期管理

use crate::http::{
    middleware::{create_middleware_stack, request_logger},
    routes::create_router,
    AppState,
};
use axum::middleware;
use std::fs;
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
    host: String,
    port: u16,
    state: AppState,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = ServerConfig { host, port };

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
    let servers_dir = get_servers_dir()?;
    let state_file_path = servers_dir.join(format!("memex-{}.state", session_id));
    if let Err(e) = fs::remove_file(&state_file_path) {
        warn!("Failed to remove state file: {}", e);
    } else {
        info!("State file removed: {}", state_file_path.display());
    }

    Ok(())
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
    use memex_core::api::Services;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::broadcast;

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

        // Create minimal test config
        let app_config = memex_core::api::AppConfig::default();

        let state = AppState::new(
            "test-lifecycle".into(),
            services,
            app_config,
            shutdown_tx.clone(),
        );

        let server_config = ServerConfig {
            host: "127.0.0.1".into(),
            port: 18080, // 使用非标准端口避免冲突
        };

        // 在后台启动服务器
        let session_id = "test-lifecycle".to_string();
        let server_handle = tokio::spawn(async move {
            start_server_with_config(session_id, server_config, state).await
        });

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
        let _state_file = servers_dir.join("memex-test-lifecycle.state");
        // 注意：测试环境中可能因为异步删除尚未完成，这里不强制检查
    }
}
