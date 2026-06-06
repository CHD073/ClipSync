use std::sync::Arc;
use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use crate::AppState;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProfileDto {
    #[serde(rename = "type")]
    pub content_type: String,
    pub hash: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub has_data: bool,
    #[serde(default)]
    pub data_name: String,
    #[serde(default)]
    pub size: i64,
}

pub async fn get(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let profile = state
        .db
        .get_latest_profile()
        .unwrap_or(ProfileDto {
            content_type: "Text".into(),
            hash: String::new(),
            text: String::new(),
            has_data: false,
            data_name: String::new(),
            size: 0,
        });

    Json(serde_json::to_value(profile).unwrap_or_default())
}

pub async fn get_latest(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let (payload, source_device_id, created_at) = state
        .db
        .get_latest_profile_with_source()
        .unwrap_or((
            ProfileDto {
                content_type: String::new(),
                hash: String::new(),
                text: String::new(),
                has_data: false,
                data_name: String::new(),
                size: 0,
            },
            String::new(),
            String::new(),
        ));
    Json(serde_json::json!({
        "payload": payload,
        "source_device_id": source_device_id,
        "created_at": created_at,
    }))
}

pub async fn put(
    State(state): State<Arc<AppState>>,
    Json(profile): Json<ProfileDto>,
) -> Json<serde_json::Value> {
    state.db.save_profile(&profile, "http-api");
    Json(serde_json::json!({ "status": "ok" }))
}
