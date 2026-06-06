use std::sync::Arc;
use axum::{
    extract::{DefaultBodyLimit, Request},
    http::StatusCode,
    middleware,
    response::Response,
};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

mod auth;
mod config;
mod db;
mod protocol;
mod routes;
mod ws;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let config = config::Config::load()?;
    let database = db::Database::open(&config.storage_path)?;
    let broadcast = ws::session::new_broadcast();

    let app_state = Arc::new(AppState {
        db: database,
        config: config.clone(),
        broadcast,
    });

    let app = routes::router()
        .route_layer(middleware::from_fn_with_state(
            app_state.clone(),
            auth_middleware,
        ))
        .route("/ws", axum::routing::get(ws::handler::ws_handler))
        .route("/health", axum::routing::get(routes::health::health_check))
        .route("/api/time", axum::routing::get(routes::api_time))
        .layer(DefaultBodyLimit::disable())
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(app_state);

    let addr = format!("0.0.0.0:{}", config.port);
    tracing::info!("listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

/// HTTP Basic Auth 中间件（保护 /SyncClipboard.json 和 /file/*）
async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    req: Request,
    next: middleware::Next,
) -> Result<Response, StatusCode> {
    auth::require_auth(req.headers(), state.config.token())?;
    Ok(next.run(req).await)
}

use axum::extract::State;

pub struct AppState {
    pub db: db::Database,
    pub config: config::Config,
    pub broadcast: ws::session::WsBroadcast,
}
