use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use crossbeam_channel::{Receiver, Sender};
use futures_util::SinkExt;
use tokio::time::sleep;
use tracing::{info, warn};

use crate::clipboard::{set_clipboard, simulate_copy, simulate_paste, ClipboardContent, ClipboardMonitor};
use crate::client::{self, WsSession};
use crate::command::{SyncCommand, SyncStatus};
use crate::config::Config;
use crate::protocol::{ProfileDto, WsServerMsg};

pub(crate) async fn run_sync(
    config: Config,
    connected: Arc<AtomicBool>,
    last_sync_at: Arc<AtomicU64>,
    uploading: Arc<AtomicBool>,
    cmd_rx: Receiver<SyncCommand>,
    status_tx: Sender<SyncStatus>,
) {
    loop {
        info!("connecting to server...");
        connected.store(false, Ordering::SeqCst);
        status_tx.send(SyncStatus {
            connected: false,
            auto_sync: config.auto_sync,
            last_sync_time: String::new(),
            last_sync_from: String::new(),
        }).ok();
        match connect_and_sync(&config, &connected, &last_sync_at, &uploading, &cmd_rx, &status_tx).await {
            Ok(()) => info!("connection ended, reconnecting in 3s"),
            Err(e) => warn!("sync error: {e:?}, reconnecting in 3s"),
        }
        connected.store(false, Ordering::SeqCst);
        sleep(Duration::from_secs(3)).await;
    }
}

async fn connect_and_sync(
    config: &Config,
    connected: &Arc<AtomicBool>,
    last_sync_at: &Arc<AtomicU64>,
    uploading: &Arc<AtomicBool>,
    cmd_rx: &Receiver<SyncCommand>,
    status_tx: &Sender<SyncStatus>,
) -> anyhow::Result<()> {
    let mut session = client::ws_connect(config).await?;
    let device_id = client::ws_auth(&mut session.writer, &mut session.reader, config).await?;
    info!("authenticated as {device_id}");
    connected.store(true, Ordering::SeqCst);
    let mut auto_sync = config.auto_sync;
    status_tx.send(SyncStatus {
        connected: true,
        auto_sync,
        last_sync_time: String::new(),
        last_sync_from: String::new(),
    }).ok();

    let mut monitor = ClipboardMonitor::new();
    let mut interval = tokio::time::interval(Duration::from_millis(100));
    let mut hb = tokio::time::interval(Duration::from_secs(30));
    hb.tick().await; // skip first immediate tick

    loop {
        tokio::select! {
            msg = client::ws_read_msg(&mut session.reader) => {
                match msg {
                    Some(WsServerMsg::ClipBroadcast { payload, source_device_id, source_device_name }) => {
                        if source_device_id == config.device_id {
                            continue;
                        }
                        if !auto_sync {
                            continue;
                        }
                        if payload.size as usize > config.auto_sync_max_bytes {
                            info!("received broadcast exceeds auto_sync_max_bytes ({} > {}), skip",
                                payload.size, config.auto_sync_max_bytes);
                            continue;
                        }
                        let from = if source_device_name.is_empty() {
                            shorten_id(&source_device_id)
                        } else {
                            source_device_name
                        };
                        info!("received broadcast: {} from {from}", payload.content_type);
                        last_sync_at.store(now_ms(), Ordering::SeqCst);
                        apply_remote_clipboard(&payload, config, &mut monitor).await;
                        status_tx.send(SyncStatus {
                            connected: true,
                            auto_sync,
                            last_sync_time: chrono::Local::now().format("%H:%M:%S").to_string(),
                            last_sync_from: from,
                        }).ok();
                    }
                    Some(WsServerMsg::Backlog { entries }) => {
                        info!("received {} backlog entries", entries.len());
                        for entry in entries {
                            if !auto_sync { break; }
                            apply_remote_clipboard(&entry, config, &mut monitor).await;
                        }
                    }
                    Some(WsServerMsg::LatestProfile { .. }) => {}
                    Some(WsServerMsg::AuthOk { .. }) => {}
                    Some(WsServerMsg::AuthError { reason }) => {
                        anyhow::bail!("auth error: {reason}");
                    }
                    None => break Ok(()),
                }
            }
            _ = interval.tick() => {
                    while let Ok(cmd) = cmd_rx.try_recv() {
                        handle_command(cmd, config, &mut session, &mut monitor, connected, last_sync_at, uploading, &mut auto_sync, status_tx).await;
                    }
                if auto_sync {
                    if let Some(content) = monitor.read_and_check() {
                        if content.size() as usize <= config.auto_sync_max_bytes {
                            last_sync_at.store(now_ms(), Ordering::SeqCst);
                            uploading.store(true, Ordering::SeqCst);
                            handle_local_clipboard(content, config, &mut session).await;
                            uploading.store(false, Ordering::SeqCst);
                        } else {
                            info!("content {} bytes exceeds auto_sync_max_bytes ({}), skipping auto push",
                                content.size(), config.auto_sync_max_bytes);
                        }
                    }
                }
            }
            _ = hb.tick() => {
                let _ = session.writer.send(tokio_tungstenite::tungstenite::Message::Ping(vec![])).await;
            }
        }
    }
}

async fn handle_command(
    cmd: SyncCommand,
    config: &Config,
    session: &mut WsSession,
    monitor: &mut ClipboardMonitor,
    connected: &Arc<AtomicBool>,
    last_sync_at: &Arc<AtomicU64>,
    uploading: &Arc<AtomicBool>,
    auto_sync: &mut bool,
    status_tx: &Sender<SyncStatus>,
) {
    match cmd {
        SyncCommand::SyncUpload => {
            last_sync_at.store(now_ms(), Ordering::SeqCst);
            uploading.store(true, Ordering::SeqCst);
            info!("manual upload triggered");
            if let Some(content) = read_clipboard_direct() {
                let hash = content.compute_hash();
                monitor.set_last_hash(&hash);
                handle_local_clipboard(content, config, session).await;
            }
            uploading.store(false, Ordering::SeqCst);
        }
        SyncCommand::SyncDownload => {
            last_sync_at.store(now_ms(), Ordering::SeqCst);
            info!("manual download triggered");
            if let Ok(latest) = client::http_get_latest(config).await {
                if !latest.payload.hash.is_empty() {
                    apply_remote_clipboard(&latest.payload, config, monitor).await;
                }
                status_tx.send(SyncStatus {
                    connected: connected.load(Ordering::SeqCst),
                    auto_sync: *auto_sync,
                    last_sync_time: latest.created_at,
                    last_sync_from: shorten_id(&latest.source_device_id),
                }).ok();
            }
        }
        SyncCommand::ToggleAutoSync => {
            *auto_sync = !*auto_sync;
            info!("auto-sync: {}", if *auto_sync { "on" } else { "off" });
            if let Ok(mut cfg) = Config::load() {
                cfg.auto_sync = *auto_sync;
                cfg.save().ok();
            }
            status_tx.send(SyncStatus {
                connected: connected.load(Ordering::SeqCst),
                auto_sync: *auto_sync,
                last_sync_time: String::new(),
                last_sync_from: String::new(),
            }).ok();
        }
        SyncCommand::CopyAndSync => {
            last_sync_at.store(now_ms(), Ordering::SeqCst);
            uploading.store(true, Ordering::SeqCst);
            info!("hotkey: copy and sync");
            simulate_copy();
            tokio::time::sleep(Duration::from_millis(200)).await;
            if let Some(content) = read_clipboard_direct() {
                let hash = content.compute_hash();
                monitor.set_last_hash(&hash);
                handle_local_clipboard(content, config, session).await;
            }
            uploading.store(false, Ordering::SeqCst);
        }
        SyncCommand::SyncAndPaste => {
            let _guard = last_sync_at.store(now_ms(), Ordering::SeqCst);
            info!("hotkey: sync and paste");
            if let Ok(latest) = client::http_get_latest(config).await {
                if !latest.payload.hash.is_empty() {
                    apply_remote_clipboard(&latest.payload, config, monitor).await;
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    simulate_paste();
                }
                status_tx.send(SyncStatus {
                    connected: connected.load(Ordering::SeqCst),
                    auto_sync: *auto_sync,
                    last_sync_time: latest.created_at,
                    last_sync_from: shorten_id(&latest.source_device_id),
                }).ok();
            }
        }
    }
}

fn read_clipboard_direct() -> Option<ClipboardContent> {
    crate::clipboard::read_clipboard()
}

fn shorten_id(id: &str) -> String {
    if id.len() > 8 {
        format!("{}…", &id[..8])
    } else {
        id.to_string()
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

async fn handle_local_clipboard(
    content: ClipboardContent,
    config: &Config,
    session: &mut WsSession,
) {
    info!("local clipboard changed: {} ({} bytes)", content.content_type(), content.size());
    let hash = content.compute_hash();
    let mut profile = ProfileDto {
        content_type: content.content_type().into(),
        hash: hash.clone(),
        text: String::new(),
        has_data: false,
        data_name: String::new(),
        size: content.size(),
    };
    match &content {
        ClipboardContent::Text(text) => {
            if content.size() as usize <= config.ws_inline_max_bytes {
                profile.text = text.clone();
            } else {
                let data_name = format!("{}.txt", hash);
                if let Err(e) = client::http_upload_file(config, &data_name, text.as_bytes()).await {
                    warn!("failed to upload text file: {e}");
                    return;
                }
                profile.has_data = true;
                profile.data_name = data_name;
            }
        }
        ClipboardContent::Image { png_bytes, .. } => {
            let data_name = format!("{}.png", hash);
            info!("uploading image: {} ({} bytes)", data_name, png_bytes.len());
            if let Err(e) = client::http_upload_file(config, &data_name, png_bytes).await {
                warn!("failed to upload image: {e}");
                return;
            }
            info!("image uploaded: {} OK", data_name);
            profile.has_data = true;
            profile.data_name = data_name;
            profile.size = png_bytes.len() as i64;
        }
        ClipboardContent::File { name, bytes } => {
            let data_name = format!("{}_{}", hash, name);
            info!("uploading file: {} ({} bytes)", data_name, bytes.len());
            if let Err(e) = client::http_upload_file(config, &data_name, bytes).await {
                warn!("failed to upload file: {e}");
                return;
            }
            info!("file uploaded: {} OK", data_name);
            profile.has_data = true;
            profile.data_name = data_name;
            profile.size = bytes.len() as i64;
        }
    }
    client::ws_send_clip(&mut session.writer, &profile, config).await;
}

async fn apply_remote_clipboard(
    payload: &ProfileDto,
    config: &Config,
    monitor: &mut ClipboardMonitor,
) {
    let (content, real_hash) = match payload.content_type.as_str() {
        "Text" => {
            let text = if payload.has_data {
                match client::http_download_file(config, &payload.data_name).await {
                    Ok(data) => String::from_utf8_lossy(&data).to_string(),
                    Err(e) => {
                        warn!("failed to download text data: {e}");
                        return;
                    }
                }
            } else {
                payload.text.clone()
            };
            let c = ClipboardContent::Text(text);
            let h = c.compute_hash();
            (c, h)
        }
        "Image" => {
            let data_name = &payload.data_name;
            info!("downloading image: {}", data_name);
            match client::http_download_file(config, &payload.data_name).await {
                Ok(data) => {
                    info!("image downloaded: {} ({} bytes)", data_name, data.len());
                    if let Some((rgba, w, h)) = decode_png_quick(&data) {
                        let img = arboard::ImageData { width: w, height: h, bytes: rgba.into() };
                        // 设置图片位图到剪贴板
                        {
                            let mut cb = match arboard::Clipboard::new() {
                                Ok(c) => c,
                                Err(e) => { warn!("failed to open clipboard: {e}"); return; }
                            };
                            if let Err(e) = cb.set_image(img) {
                                warn!("failed to set image: {e}");
                            }
                        }
                        // 同时保存 PNG 并设置 CF_HDROP，允许粘贴为文件
                        let c = ClipboardContent::File {
                            name: format!("{}.png", &payload.hash[..8]),
                            bytes: data.clone(),
                        };
                        set_clipboard(&c);
                        let h = c.compute_hash();
                        monitor.set_last_hash(&h);
                        info!("set clipboard: Image + File {}", h);
                        return;
                    }
                    warn!("failed to decode received image PNG");
                    return;
                }
                Err(e) => { warn!("failed to download image data: {e}"); return; }
            }
        }
        "File" => {
            info!("downloading file: {}", payload.data_name);
            match client::http_download_file(config, &payload.data_name).await {
                Ok(data) => {
                    // data_name format: hash_originalname.ext
                    let name = payload.data_name.find('_')
                        .map(|i| &payload.data_name[i+1..])
                        .unwrap_or(&payload.data_name);
                    let c = ClipboardContent::File { name: name.to_string(), bytes: data.clone() };
                    let h = c.compute_hash();
                    set_clipboard(&c);
                    monitor.set_last_hash(&h);
                    info!("got file: {} ({} bytes)", name, data.len());
                    return;
                }
                Err(e) => { warn!("failed to download file: {e}"); return; }
            }
        }
        _ => { warn!("unknown content type: {}", payload.content_type); return; }
    };
    set_clipboard(&content);
    monitor.set_last_hash(&real_hash);
    info!("set clipboard: {} {}", payload.content_type, real_hash);
}

fn decode_png_quick(data: &[u8]) -> Option<(Vec<u8>, usize, usize)> {
    let img = image::load_from_memory(data).ok()?;
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    Some((rgba.into_raw(), w as usize, h as usize))
}


