use std::sync::Arc;
use axum::{
    body::Bytes,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use tracing::error;
use crate::AppState;

pub async fn get(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let path = match sanitize_path(&name, &state.config.storage_path) {
        Some(p) => p,
        None => return Err(StatusCode::BAD_REQUEST),
    };

    let data = tokio::fs::read(&path).await.map_err(|e| {
        error!("file GET failed: {e}");
        if e.kind() == std::io::ErrorKind::NotFound {
            StatusCode::NOT_FOUND
        } else {
            StatusCode::INTERNAL_SERVER_ERROR
        }
    })?;
    Ok(([("Content-Type", "application/octet-stream")], data))
}

pub async fn put(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    body: Bytes,
) -> StatusCode {
    let path = match sanitize_path(&name, &state.config.storage_path) {
        Some(p) => p,
        None => return StatusCode::BAD_REQUEST,
    };

    let dir = path.parent().unwrap_or(&state.config.storage_path);
    if let Err(e) = tokio::fs::create_dir_all(dir).await {
        error!("file PUT create dir failed: {e}");
        return StatusCode::INTERNAL_SERVER_ERROR;
    }
    if let Err(e) = tokio::fs::write(&path, &body).await {
        error!("file PUT write failed: {e}");
        return StatusCode::INTERNAL_SERVER_ERROR;
    }

    StatusCode::OK
}

fn sanitize_path(name: &str, base: &std::path::Path) -> Option<std::path::PathBuf> {
    // Reject empty names, reserved Windows names, names starting with dot
    if name.is_empty()
        || name == "." || name == ".."
        || name.contains('/') || name.contains('\\')
        || name.contains('\0')
        || name.len() > 255
    {
        return None;
    }
    // Reject Windows reserved names
    let upper = name.to_uppercase();
    let reserved = ["CON", "PRN", "AUX", "NUL", "COM1","COM2","COM3","COM4",
                     "COM5","COM6","COM7","COM8","COM9",
                     "LPT1","LPT2","LPT3","LPT4","LPT5","LPT6","LPT7","LPT8","LPT9"];
    if reserved.iter().any(|r| upper == *r || upper.starts_with(r) && upper.as_bytes().get(3) == Some(&b'.')) {
        return None;
    }
    if reserved.contains(&upper.as_str()) {
        return None;
    }

    let path = base.join("files").join(name);
    // Ensure the resolved path is still within the base directory
    if path.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
        return None;
    }
    Some(path)
}
