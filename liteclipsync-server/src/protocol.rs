use serde::{Deserialize, Serialize};

use crate::routes::sync_profile::ProfileDto;

/// 客户端 → 服务端 WS 消息
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WsClientMsg {
    Auth {
        token: String,
        device_id: String,
        #[serde(default)]
        name: Option<String>,
    },
    LiteClipSync {
        payload: ProfileDto,
        device_id: String,
    },
    GetLatest,
}

/// 服务端 → 客户端 WS 消息
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WsServerMsg {
    AuthOk {
        device_id: String,
    },
    AuthError {
        reason: String,
    },
    ClipBroadcast {
        payload: ProfileDto,
        source_device_id: String,
        #[serde(default)]
        source_device_name: String,
    },
    Backlog {
        entries: Vec<ProfileDto>,
    },
    LatestProfile {
        payload: ProfileDto,
        source_device_id: String,
        created_at: String,
    },
}
