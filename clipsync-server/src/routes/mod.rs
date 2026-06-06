use std::sync::Arc;
use axum::{Router, routing::get};
use crate::AppState;

pub(crate) mod health;
pub(crate) mod sync_profile;
pub(crate) mod file;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/SyncClipboard.json", get(sync_profile::get).put(sync_profile::put))
        .route("/file/{name}", get(file::get).put(file::put))
        .route("/profile/latest", get(sync_profile::get_latest))
}

pub async fn api_time() -> axum::Json<serde_json::Value> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    axum::Json(serde_json::json!({ "time": secs }))
}
