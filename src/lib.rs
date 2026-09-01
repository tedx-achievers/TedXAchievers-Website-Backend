use axum::{
    http::{header::CONTENT_TYPE, HeaderValue, Method, StatusCode},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    limit::RequestBodyLimitLayer,
    timeout::TimeoutLayer,
    trace::TraceLayer,
};
use tracing::info;

pub mod config;
pub mod errors;
pub mod middleware;
pub mod models;
pub mod modules;
pub mod utils;

use config::{db::connect_db, Config};
use errors::AppError;

#[derive(Clone)]
pub struct AppState {
    pub db: mongodb::Database,
    pub cache: Arc<DashMap<String, middleware::auth::CachedAuthUser>>,
    pub rate_limits: Arc<DashMap<String, std::collections::VecDeque<std::time::Instant>>>,
    pub verification_resends: Arc<DashMap<String, Instant>>,
    pub email_queue: tokio::sync::mpsc::Sender<utils::email::EmailJob>,
    pub config: Arc<Config>,
}

async fn not_found() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({"success": false, "message": "Route not found"})),
    )
}

async fn log_failed_requests(
    request: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let response = next.run(request).await;
    if response.status().is_server_error() {
        tracing::error!(%method, %uri, status = %response.status(), "HTTP request returned server error");
    }
    response
}

pub async fn build_app_state(config: Arc<Config>) -> AppState {
    let db = connect_db(&config).await;
    let email_queue = utils::email::start_worker(Arc::clone(&config));
    AppState {
        db,
        cache: Arc::new(DashMap::new()),
        rate_limits: Arc::new(DashMap::new()),
        verification_resends: Arc::new(DashMap::new()),
        email_queue,
        config,
    }
}

pub fn build_router(state: AppState) -> Router {
    let state = Arc::new(state);
    let allowed_origins = state
        .config
        .frontend_urls
        .iter()
        .filter_map(|origin| HeaderValue::from_str(origin).ok())
        .collect::<Vec<_>>();
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list(allowed_origins))
        .allow_methods([Method::GET, Method::POST, Method::PATCH])
        .allow_headers([CONTENT_TYPE])
        .allow_credentials(true);
    let middleware_state = Arc::clone(&state);
    Router::new()
        .route("/api/health", get(modules::health::handler::health_handler))
        .nest("/api/auth", modules::auth::router())
        .nest("/api/tickets", modules::tickets::router())
        .nest("/api/volunteers", modules::volunteers::router())
        .nest("/api/admin", modules::admin::router())
        .nest("/api/dashboard", modules::dashboard::router())
        .fallback(not_found)
        .with_state(state)
        .layer(cors)
        .layer(RequestBodyLimitLayer::new(32 * 1024))
        .layer(TimeoutLayer::new(Duration::from_secs(30)))
        .layer(axum::middleware::from_fn_with_state(
            middleware_state,
            middleware::rate_limit::request_rate_limit,
        ))
        .layer(axum::middleware::from_fn(log_failed_requests))
        .layer(TraceLayer::new_for_http())
}

pub async fn run() -> Result<(), AppError> {
    let _ = dotenvy::dotenv();
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let config = Arc::new(Config::from_env());
    let address = format!("0.0.0.0:{}", config.port)
        .parse::<std::net::SocketAddr>()
        .map_err(|error| AppError::Internal(anyhow::anyhow!(error)))?;
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .map_err(|error| AppError::Internal(anyhow::anyhow!(error)))?;
    info!(
        "TCP listener bound on {}",
        listener
            .local_addr()
            .map_err(|error| AppError::Internal(anyhow::anyhow!(error)))?
    );
    let state = build_app_state(config).await;
    let app = build_router(state);
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await
    .map_err(|error| AppError::Internal(anyhow::anyhow!(error)))
}
