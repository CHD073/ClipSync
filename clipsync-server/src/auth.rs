use axum::http::{HeaderMap, StatusCode};
use base64::Engine;

pub fn verify_auth(headers: &HeaderMap, expected_token: &str) -> bool {
    extract_token(headers).is_some_and(|t| t == expected_token)
}

pub fn require_auth(headers: &HeaderMap, expected_token: &str) -> Result<(), StatusCode> {
    if verify_auth(headers, expected_token) {
        Ok(())
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

/// 从 WS Auth 消息中验证 token
pub fn verify_ws_token(msg_token: &str, expected_token: &str) -> bool {
    // 支持两种格式：原始 token 或 base64(username:token)
    if msg_token == expected_token {
        return true;
    }
    if let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(msg_token) {
        if let Ok(s) = String::from_utf8(decoded) {
            if s.split(':').next().is_some_and(|t| t == expected_token) {
                return true;
            }
        }
    }
    false
}

fn extract_token(headers: &HeaderMap) -> Option<String> {
    let header = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())?;

    let encoded = header.strip_prefix("Basic ")?;
    let bytes = base64::engine::general_purpose::STANDARD.decode(encoded).ok()?;
    let s = String::from_utf8(bytes).ok()?;
    s.split(':').next().map(|s| s.to_string())
}
