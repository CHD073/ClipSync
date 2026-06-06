use anyhow::Context;
use futures_util::StreamExt;
use reqwest::Client as HttpClient;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use tracing::{error, info, warn};

fn http(config: &Config) -> HttpClient {
    HttpClient::builder()
        .timeout(std::time::Duration::from_secs(config.http_timeout_secs))
        .build()
        .unwrap()
}

use serde::Deserialize;
use crate::config::Config;
use crate::protocol::{ProfileDto, WsClientMsg, WsServerMsg};

#[derive(Debug, Deserialize)]
pub(crate) struct LatestProfileResponse {
    pub payload: ProfileDto,
    pub source_device_id: String,
    pub created_at: String,
}

pub struct WsSession {
    pub writer: futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
        Message,
    >,
    pub reader: futures_util::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    >,
}

pub async fn http_upload_file(
    config: &Config,
    data_name: &str,
    data: &[u8],
) -> anyhow::Result<()> {
    let url = format!("{}/file/{}", config.api_base(), data_name);
    let client = http(config);
    let resp = client
        .put(&url)
        .header("Authorization", &config.basic_auth_value())
        .body(data.to_vec())
        .send()
        .await
        .context("file upload failed")?;
    anyhow::ensure!(resp.status().is_success(), "upload {} -> {}", data_name, resp.status());
    info!("http upload: {} ({} bytes) {}", data_name, data.len(), resp.status());
    Ok(())
}

pub async fn http_download_file(
    config: &Config,
    data_name: &str,
) -> anyhow::Result<Vec<u8>> {
    let url = format!("{}/file/{}", config.api_base(), data_name);
    let client = http(config);
    let resp = client
        .get(&url)
        .header("Authorization", &config.basic_auth_value())
        .send()
        .await
        .context("file download failed")?;
    anyhow::ensure!(resp.status().is_success(), "download {} -> {}", data_name, resp.status());
    let data = resp.bytes().await?.to_vec();
    info!("http download: {} ({} bytes)", data_name, data.len());
    Ok(data)
}

pub async fn http_get_latest(config: &Config) -> anyhow::Result<LatestProfileResponse> {
    let url = format!("{}/profile/latest", config.api_base());
    let client = http(config);
    let resp = client
        .get(&url)
        .header("Authorization", &config.basic_auth_value())
        .send()
        .await
        .context("failed to fetch latest profile")?;
    let data: LatestProfileResponse = resp
        .json()
        .await
        .context("failed to parse latest profile response")?;
    Ok(data)
}

pub async fn ws_connect(config: &Config) -> anyhow::Result<WsSession> {
    let url = config.ws_url();
    info!("connecting to {url}");

    let (ws_stream, _) = connect_async(&url)
        .await
        .context("WebSocket connection failed")?;

    let (writer, reader) = ws_stream.split();
    Ok(WsSession { writer, reader })
}

pub async fn ws_auth(
    writer: &mut (impl futures_util::SinkExt<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin),
    reader: &mut (impl StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin),
    config: &Config,
) -> anyhow::Result<String> {
    let auth_msg = WsClientMsg::Auth {
        token: config.token.clone(),
        device_id: config.device_id.clone(),
        name: Some(config.device_name.clone()),
    };
    let auth_json = serde_json::to_string(&auth_msg)?;
    writer.send(Message::Text(auth_json.into())).await?;

    match reader.next().await {
        Some(Ok(Message::Text(resp))) => {
            let server_msg: WsServerMsg = serde_json::from_str(&resp)?;
            match server_msg {
                WsServerMsg::AuthOk { device_id } => {
                    info!("auth ok, device_id: {device_id}");
                    Ok(device_id)
                }
                WsServerMsg::AuthError { reason } => {
                    anyhow::bail!("auth error: {reason}");
                }
                _ => {
                    anyhow::bail!("unexpected response: {resp}");
                }
            }
        }
        Some(Ok(other)) => anyhow::bail!("unexpected ws message: {other:?}"),
        Some(Err(e)) => anyhow::bail!("ws error during auth: {e}"),
        None => anyhow::bail!("connection closed during auth"),
    }
}

pub async fn ws_send_clip(
    writer: &mut (impl futures_util::SinkExt<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin),
    profile: &ProfileDto,
    config: &Config,
) {
    let msg = WsClientMsg::ClipSync {
        payload: profile.clone(),
        device_id: config.device_id.clone(),
    };
    let json = match serde_json::to_string(&msg) {
        Ok(j) => j,
        Err(e) => {
            error!("failed to serialize ClipSync: {e}");
            return;
        }
    };
    if let Err(e) = writer.send(Message::Text(json.into())).await {
        warn!("failed to send ClipSync: {e}");
    } else {
        info!("sent clip: {} {}", profile.content_type, profile.hash);
    }
}

pub async fn ws_read_msg(
    reader: &mut (impl StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin),
) -> Option<WsServerMsg> {
    loop {
        match reader.next().await {
            Some(Ok(Message::Text(text))) => {
                match serde_json::from_str::<WsServerMsg>(&text) {
                    Ok(msg) => return Some(msg),
                    Err(e) => {
                        warn!("failed to parse server message: {e}: {text}");
                        continue;
                    }
                }
            }
            Some(Ok(Message::Ping(_) | Message::Pong(_))) => continue,
            Some(Ok(Message::Close(_))) => {
                info!("server closed connection");
                return None;
            }
            Some(Ok(Message::Binary(_) | Message::Frame(_))) => {
                warn!("unexpected ws message type");
                continue;
            }
            Some(Err(e)) => {
                warn!("ws read error: {e}");
                return None;
            }
            None => {
                info!("ws stream ended");
                return None;
            }
        }
    }
}
