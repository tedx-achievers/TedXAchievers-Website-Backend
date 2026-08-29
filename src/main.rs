use axum::{
    http::{header::CONTENT_TYPE, HeaderValue, Method, StatusCode},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use dashmap::DashMap;
use std::{net::SocketAddr, sync::Arc};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
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
    pub cache: Arc<DashMap<String, serde_json::Value>>,
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
    let state = Arc::new(AppState {
        db,
        cache: Arc::new(DashMap::new()),
        config: Arc::clone(&config),
    });
    let origin = HeaderValue::from_str(&config.frontend_url)
        .map_err(|error| AppError::Internal(anyhow::anyhow!(error)))?;
    let cors = CorsLayer::new()
        .allow_origin(origin)
        .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::DELETE])
        .allow_headers([CONTENT_TYPE])
        .allow_credentials(true);
    let app = Router::new()
        .route("/api/health", get(modules::health::handler::health_handler))
        .nest("/api/auth", modules::auth::router())
        .nest("/api/tickets", modules::tickets::router())
        .nest("/api/volunteers", modules::volunteers::router())
        .nest("/api/admin", modules::admin::router())
        .fallback(not_found)
        .with_state(state)
        .layer(cors)
        .layer(TraceLayer::new_for_http());
    info!(
        "TEDxAchievers API listening on {}",
        listener
            .local_addr()
            .map_err(|error| AppError::Internal(anyhow::anyhow!(error)))?
    );
    axum::serve(listener, app)
        .await
        .map_err(|error| AppError::Internal(anyhow::anyhow!(error)))
}
