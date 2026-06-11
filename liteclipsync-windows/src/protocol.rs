use serde::{Deserialize, Serialize};

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
