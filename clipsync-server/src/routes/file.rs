use std::sync::Arc;
use axum::{
    body::Bytes,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use crate::AppState;

pub async fn get(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let name = name.replace("..", "").replace('/', "_").replace('\\', "_");
    let path = state.config.storage_path.join("files").join(&name);

    if !path.exists() {
        return Err(StatusCode::NOT_FOUND);
    }

    let data = tokio::fs::read(&path).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(([("Content-Type", "application/octet-stream")], data))
}

pub async fn put(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    body: Bytes,
) -> StatusCode {
    let name = name.replace("..", "").replace('/', "_").replace('\\', "_");
    let dir = state.config.storage_path.join("files");
    let path = dir.join(&name);

    tokio::fs::create_dir_all(&dir).await.ok();
    tokio::fs::write(&path, &body).await.ok();

    StatusCode::OK
}
