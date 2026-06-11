use std::sync::Arc;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use crate::auth;
use crate::protocol::{WsClientMsg, WsServerMsg};
use crate::routes::sync_profile::ProfileDto;
use crate::AppState;
use super::session::WsMessage;

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: Arc<AppState>) {
    let (mut ws_sender, mut ws_receiver) = socket.split();

    // ── Phase 1: 鉴权 ──
    let (device_id, device_name) = match authenticate(&mut ws_receiver, &mut ws_sender, &state).await {
        Some(v) => v,
        None => return,
    };

    tracing::info!("device authenticated: {}", device_id);

    // 注册 / 更新设备在线时间
    state.db.register_device(&device_id, device_name.as_deref().unwrap_or_default());

    // ── Phase 2: 发送离线 Backlog ──
    let backlog = state.db.get_backlog(&device_id);
    if !backlog.is_empty() {
        let msg = WsServerMsg::Backlog { entries: backlog };
        if let Ok(json) = serde_json::to_string(&msg) {
            let _ = ws_sender.send(Message::Text(json.into())).await;
        }
    }

    // ── Phase 3: 主循环 ──
    let broadcast = state.broadcast.clone();
    let mut rx = broadcast.subscribe();
    let my_device_id = device_id.clone();

    let mut send_handle = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if let Some(target) = &msg.target_device_id {
                if target != &my_device_id {
                    continue;
                }
            } else {
                if msg.source_device_id.as_deref() == Some(&my_device_id) {
                    continue;
                }
            }
            if ws_sender.send(Message::Text(msg.data.into())).await.is_err() {
                break;
            }
        }
    });

    let did = device_id.clone();
    let mut recv_handle = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_receiver.next().await {
            match msg {
                Message::Text(text) => {
                    handle_client_message(&text, &did, &state, &broadcast).await;
                }
                _ => {}
            }
        }
    });

    tokio::select! {
        _ = &mut send_handle => { recv_handle.abort(); }
        _ = &mut recv_handle => { send_handle.abort(); }
    }

    tracing::info!("device disconnected: {}", device_id);
}

// ── 鉴权流程 ──
async fn authenticate(
    receiver: &mut (impl StreamExt<Item = Result<Message, axum::Error>> + Unpin),
    sender: &mut (impl SinkExt<Message> + Unpin),
    state: &AppState,
) -> Option<(String, Option<String>)> {
    match receiver.next().await {
        Some(Ok(Message::Text(text))) => {
            let msg: WsClientMsg = serde_json::from_str(&text).ok()?;

            match msg {
                WsClientMsg::Auth { token, device_id, name } => {
                    if !auth::verify_ws_token(&token, state.config.token()) {
                        let err = WsServerMsg::AuthError {
                            reason: "invalid token".into(),
                        };
                        if let Ok(json) = serde_json::to_string(&err) {
                            let _ = sender.send(Message::Text(json.into())).await;
                        }
                        return None;
                    }
                    let ok = WsServerMsg::AuthOk {
                        device_id: device_id.clone(),
                    };
                    if let Ok(json) = serde_json::to_string(&ok) {
                        let _ = sender.send(Message::Text(json.into())).await;
                    }
                    Some((device_id, name))
                }
                _ => {
                    let err = WsServerMsg::AuthError {
                        reason: "first message must be Auth".into(),
                    };
                    if let Ok(json) = serde_json::to_string(&err) {
                        let _ = sender.send(Message::Text(json.into())).await;
                    }
                    None
                }
            }
        }
        _ => None,
    }
}

// ── 处理客户端消息 ──
async fn handle_client_message(
    text: &str,
    my_device_id: &str,
    state: &AppState,
    broadcast: &super::session::WsBroadcast,
) {
    let msg: WsClientMsg = match serde_json::from_str(text) {
        Ok(m) => m,
        Err(_) => return,
    };

    match msg {
        WsClientMsg::LiteClipSync { payload, device_id } => {
            // 保存到离线队列
            if !state.db.save_profile(&payload, &device_id) {
                tracing::error!("WS save_profile failed, dropping payload");
                return;
            }

            // 查设备名
            let source_name = state.db.get_device_name(&device_id).unwrap_or_default();

            // 序列化广播消息
            let bcast = WsServerMsg::ClipBroadcast {
                payload,
                source_device_id: device_id,
                source_device_name: source_name,
            };
            if let Ok(json) = serde_json::to_string(&bcast) {
                let _ = broadcast.send(WsMessage {
                    data: json,
                    source_device_id: Some(my_device_id.to_string()),
                    target_device_id: None,
                });
            }
        }
        WsClientMsg::GetLatest => {
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
            let resp = WsServerMsg::LatestProfile {
                payload,
                source_device_id,
                created_at,
            };
            if let Ok(json) = serde_json::to_string(&resp) {
                let _ = broadcast.send(WsMessage {
                    data: json,
                    source_device_id: Some(my_device_id.to_string()),
                    target_device_id: Some(my_device_id.to_string()),
                });
            }
        }
        WsClientMsg::Auth { .. } => {
            // 已鉴权后收到重复 Auth 忽略
        }
    }
}
