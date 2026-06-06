use axum::Json;
use serde_json::Value;

pub async fn health_check() -> Json<Value> {
    Json(serde_json::json!({
        "status": "ok",
        "service": "clipsync-server",
        "version": env!("CARGO_PKG_VERSION")
    }))
}
