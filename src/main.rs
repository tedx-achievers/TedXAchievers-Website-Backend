use axum::{
    http::{header::CONTENT_TYPE, HeaderValue, Method, StatusCode},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use dashmap::DashMap;
use std::{net::SocketAddr, sync::Arc};
use std::time::Duration;
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    limit::RequestBodyLimitLayer,
    timeout::TimeoutLayer,
    trace::TraceLayer,
};
use tracing::info;
mod config;
mod errors;
mod middleware;
mod models;
mod modules;
mod utils;
use config::{db::connect_db, Config};
use errors::AppError;
#[derive(Clone)]
pub struct AppState {
    pub db: mongodb::Database,
    pub cache: Arc<DashMap<String, middleware::auth::CachedAuthUser>>,
    pub rate_limits: Arc<DashMap<String, std::collections::VecDeque<std::time::Instant>>>,
    pub email_queue: tokio::sync::mpsc::Sender<utils::email::EmailJob>,
    pub config: Arc<Config>,
}
async fn not_found() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({"success": false, "message": "Route not found"})),
    )
}
#[tokio::main]
async fn main() -> Result<(), AppError> {
    let _ = dotenvy::dotenv();
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let config = Arc::new(Config::from_env());
    let address: SocketAddr = format!("0.0.0.0:{}", config.port)
        .parse::<SocketAddr>()
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
    let db = connect_db(&config).await;
    let email_queue = utils::email::start_worker(Arc::clone(&config));
    let state = Arc::new(AppState {
        db,
        cache: Arc::new(DashMap::new()),
        rate_limits: Arc::new(DashMap::new()),
        email_queue,
        config: Arc::clone(&config),
    });
    let allowed_origins = config
        .frontend_urls
        .iter()
        .map(|origin| {
            HeaderValue::from_str(origin)
                .map_err(|error| AppError::Internal(anyhow::anyhow!(error)))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list(allowed_origins))
        .allow_methods([Method::GET, Method::POST, Method::PATCH])
        .allow_headers([CONTENT_TYPE])
        .allow_credentials(true);
    let middleware_state = Arc::clone(&state);
    let app = Router::new()
        .route("/api/health", get(modules::health::handler::health_handler))
        .nest("/api/auth", modules::auth::router())
        .nest("/api/tickets", modules::tickets::router())
        .nest("/api/volunteers", modules::volunteers::router())
        .nest("/api/admin", modules::admin::router())
        .fallback(not_found)
        .with_state(state)
        .layer(cors)
        .layer(RequestBodyLimitLayer::new(32 * 1024))
        .layer(TimeoutLayer::new(Duration::from_secs(30)))
        .layer(axum::middleware::from_fn_with_state(
            middleware_state,
            middleware::rate_limit::request_rate_limit,
        ))
        .layer(TraceLayer::new_for_http());
    info!(
        "TEDxAchievers API listening on {}",
        listener
            .local_addr()
            .map_err(|error| AppError::Internal(anyhow::anyhow!(error)))?
    );
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
        .await
        .map_err(|error| AppError::Internal(anyhow::anyhow!(error)))
}
