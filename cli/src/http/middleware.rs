//! HTTP中间件配置

use axum::{
    body::Body,
    http::{header, HeaderValue, Method, Request},
    middleware::Next,
    response::Response,
};
use std::time::{Duration, Instant};
use tower_http::{cors::CorsLayer, timeout::TimeoutLayer, trace::TraceLayer};
use tracing::{info, warn};

/// 创建中间件栈
pub fn create_middleware_stack() -> tower::layer::util::Stack<CorsLayer, TimeoutLayer> {
    tower::layer::util::Stack::new(create_cors_layer(), create_timeout_layer())
}

/// 创建CORS中间件 - 仅允许localhost
fn create_cors_layer() -> CorsLayer {
    // 使用函数来验证 origin 是否为 localhost
    CorsLayer::new()
        .allow_origin(tower_http::cors::AllowOrigin::predicate(
            |origin: &HeaderValue, _| {
                origin
                    .to_str()
                    .map(|s| {
                        s.starts_with("http://localhost")
                            || s.starts_with("https://localhost")
                            || s.starts_with("http://127.0.0.1")
                            || s.starts_with("https://127.0.0.1")
                    })
                    .unwrap_or(false)
            },
        ))
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION, header::ACCEPT])
        .allow_credentials(true)
        .max_age(Duration::from_secs(3600))
}

/// 创建超时中间件 - 30秒
fn create_timeout_layer() -> TimeoutLayer {
    TimeoutLayer::new(Duration::from_secs(30))
}

/// 创建请求日志layer（用于HTTP请求追踪）
pub fn create_trace_layer(
) -> TraceLayer<tower_http::classify::SharedClassifier<tower_http::classify::ServerErrorsAsFailures>>
{
    TraceLayer::new_for_http()
}

/// 请求日志中间件（手动实现，用于记录详细信息）
pub async fn request_logger(req: Request<Body>, next: Next) -> Response {
    let method = req.method().clone();
    let uri = req.uri().clone();
    let start = Instant::now();

    // 执行请求
    let response = next.run(req).await;

    let duration = start.elapsed();
    let status = response.status();

    // 根据状态码选择日志级别
    if status.is_success() {
        info!(
            method = %method,
            uri = %uri,
            status = %status.as_u16(),
            duration_ms = %duration.as_millis(),
            "Request completed"
        );
    } else if status.is_client_error() || status.is_server_error() {
        warn!(
            method = %method,
            uri = %uri,
            status = %status.as_u16(),
            duration_ms = %duration.as_millis(),
            "Request failed"
        );
    } else {
        info!(
            method = %method,
            uri = %uri,
            status = %status.as_u16(),
            duration_ms = %duration.as_millis(),
            "Request completed"
        );
    }

    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        middleware,
        response::IntoResponse,
        routing::get,
        Router,
    };
    use std::time::Duration;
    use tower::ServiceExt;

    async fn test_handler() -> impl IntoResponse {
        "OK"
    }

    async fn slow_handler() -> impl IntoResponse {
        tokio::time::sleep(Duration::from_millis(100)).await;
        "Slow response"
    }

    #[tokio::test]
    async fn test_cors_localhost_allowed() {
        let app = Router::new()
            .route("/test", get(test_handler))
            .layer(create_cors_layer());

        // 测试 localhost origin
        let request = Request::builder()
            .method(Method::GET)
            .uri("/test")
            .header("Origin", "http://localhost:3000")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        // 检查CORS头
        let cors_header = response
            .headers()
            .get("access-control-allow-origin")
            .and_then(|v| v.to_str().ok());

        assert!(cors_header.is_some());
        assert!(
            cors_header.unwrap().contains("localhost")
                || cors_header.unwrap().contains("127.0.0.1")
        );
    }

    #[tokio::test]
    async fn test_cors_preflight() {
        let app = Router::new()
            .route("/test", get(test_handler))
            .layer(create_cors_layer());

        // 测试 OPTIONS preflight 请求
        let request = Request::builder()
            .method(Method::OPTIONS)
            .uri("/test")
            .header("Origin", "http://localhost:3000")
            .header("Access-Control-Request-Method", "POST")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        // preflight 请求应该返回成功状态
        assert!(response.status().is_success() || response.status() == StatusCode::NO_CONTENT);

        // 检查必要的CORS头
        assert!(response
            .headers()
            .get("access-control-allow-methods")
            .is_some());
    }

    #[tokio::test]
    async fn test_request_logger() {
        let app = Router::new()
            .route("/test", get(test_handler))
            .layer(middleware::from_fn(request_logger));

        let request = Request::builder()
            .method(Method::GET)
            .uri("/test")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        // 验证请求成功
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_timeout_layer() {
        let app = Router::new()
            .route("/slow", get(slow_handler))
            .layer(create_timeout_layer());

        let request = Request::builder()
            .method(Method::GET)
            .uri("/slow")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        // 短延迟应该成功（未超时）
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_cors_allow_methods() {
        let cors_layer = create_cors_layer();

        // 验证允许的方法
        let app = Router::new()
            .route("/test", get(test_handler))
            .layer(cors_layer);

        for method in [Method::GET, Method::POST, Method::PUT, Method::DELETE] {
            let request = Request::builder()
                .method(Method::OPTIONS)
                .uri("/test")
                .header("Origin", "http://localhost:3000")
                .header("Access-Control-Request-Method", method.as_str())
                .body(Body::empty())
                .unwrap();

            let response = app.clone().oneshot(request).await.unwrap();

            // preflight 应该成功
            assert!(response.status().is_success() || response.status() == StatusCode::NO_CONTENT);
        }
    }

    #[tokio::test]
    async fn test_middleware_stack() {
        let app = Router::new()
            .route("/test", get(test_handler))
            .layer(create_middleware_stack());

        let request = Request::builder()
            .method(Method::GET)
            .uri("/test")
            .header("Origin", "http://localhost:3000")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        // 验证中间件栈正常工作
        assert_eq!(response.status(), StatusCode::OK);
    }
}
