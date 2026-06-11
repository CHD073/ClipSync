#![windows_subsystem = "windows"]

mod clipboard;
mod client;
mod command;
mod config;
mod protocol;
mod sync;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use clap::Parser;
use crossbeam_channel::{bounded, unbounded};
use tracing_subscriber::EnvFilter;
use tray_icon::{
    TrayIconBuilder,
    menu::{Menu, MenuItem, IconMenuItem, Submenu, CheckMenuItem, PredefinedMenuItem, MenuEvent, MenuId, Icon as MenuIcon},
    Icon,
};

use command::{SyncCommand, SyncStatus};

#[derive(Parser)]
#[command(name = "clipsync", about = "ClipSync Windows client")]
struct Cli {
    #[arg(short, long, help = "Override server URL")]
    server: Option<String>,
    #[arg(short, long, help = "Override auth token")]
    token: Option<String>,
}

fn main() {
    // ── 单实例保护 ──
    unsafe {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::System::Threading::CreateMutexW;
        use windows_sys::Win32::Foundation::GetLastError;
        const ERROR_ALREADY_EXISTS: u32 = 183;
        let name = "Global\\ClipSync\0";
        let wide: Vec<u16> = OsStr::new(name).encode_wide().collect();
        let handle = CreateMutexW(std::ptr::null_mut(), 1, wide.as_ptr());
        if handle.is_null() || GetLastError() == ERROR_ALREADY_EXISTS {
            return;
        }
    }

    match std::fs::File::create("clipsync.log") {
        Ok(file) => {
            tracing_subscriber::fmt()
                .with_ansi(false)
                .with_writer(Arc::new(file))
                .with_env_filter(
                    EnvFilter::try_from_default_env()
                        .unwrap_or_else(|_| EnvFilter::new("info")),
                )
                .init();
        }
        Err(e) => {
            tracing_subscriber::fmt()
                .with_ansi(false)
                .with_env_filter(
                    EnvFilter::try_from_default_env()
                        .unwrap_or_else(|_| EnvFilter::new("info")),
                )
                .init();
            tracing::warn!("could not create log file: {e}, logging to stderr");
        }
    }

    let cli = Cli::parse();
    let mut cfg = config::Config::load().expect("failed to load config");
    if let Some(url) = cli.server {
        cfg.server_url = url;
    }
    if let Some(token) = cli.token {
        cfg.token = token;
    }

    tracing::info!(
        "ClipSync starting: server={}, device={} ({})",
        cfg.server_url, cfg.device_name, cfg.device_id
    );

    let running = Arc::new(AtomicBool::new(true));
    let shutdown = Arc::new(AtomicBool::new(false));
    let connected = Arc::new(AtomicBool::new(false));
    let last_sync_at = Arc::new(AtomicU64::new(0));
    let uploading = Arc::new(AtomicBool::new(false));

    let (cmd_tx, cmd_rx) = unbounded::<SyncCommand>();
    let (status_tx, status_rx) = bounded::<SyncStatus>(16);

    // ── 第一时间创建托盘 (消除启动鼠标转圈) ──
    let _menu = build_menu(&cfg, false, "", "", false, false, &cmd_tx, &cfg.language);
    let tray = match TrayIconBuilder::new()
        .with_tooltip(if cfg.language == "zh" { "ClipSync - 连接中..." } else { "ClipSync - connecting..." })
        .with_icon(make_circle_icon(0xFF, 0xCC, 0x00))
        .with_menu(Box::new(_menu))
        .build()
    {
        Ok(t) => t,
        Err(e) => {
            tracing::error!("failed to create tray icon: {e}, exiting");
            return;
        }
    };

    // ── 注册全局热键 ──
    register_hotkeys(&cfg);

    // ── 创建隐藏窗口，告诉 Windows 程序已就绪（消除启动光标转圈）──
    unsafe {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::UI::WindowsAndMessaging::*;
        let class: Vec<u16> = OsStr::new("STATIC\0").encode_wide().collect();
        let title: Vec<u16> = OsStr::new("ClipSync Hidden\0").encode_wide().collect();
        CreateWindowExW(
            0,
            class.as_ptr(),
            title.as_ptr(),
            0,
            0, 0, 0, 0,
            HWND_MESSAGE,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null(),
        );
    }
    let r = running.clone();
    let sd = shutdown.clone();
    let c = connected.clone();
    let s = last_sync_at.clone();
    let u = uploading.clone();
    let cfg_clone = cfg.clone();
    std::thread::spawn(move || {
        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(e) => {
                tracing::error!("failed to start runtime: {e}");
                return;
            }
        };
        rt.block_on(async {
            sync::run_sync(cfg_clone, c, s, u, cmd_rx, status_tx, sd).await;
        });
        r.store(false, Ordering::SeqCst);
    });

    let menu_rx = MenuEvent::receiver();
    let mut icon_state: &str = "connecting";
    let mut last_icon_update = std::time::Instant::now();
    let mut menu_dirty = true;
    let mut last_sync_time = String::new();
    let mut last_sync_from = String::new();
    let mut auto_sync = cfg.auto_sync;
    let mut last_icon_color: Option<(u8, u8, u8)> = None;
    let mut last_tooltip: Option<String> = None;

    unsafe {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            DispatchMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE,
            WM_HOTKEY,
        };

        while running.load(Ordering::SeqCst) {
            let mut msg = std::mem::zeroed::<MSG>();
            while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                if msg.message == WM_HOTKEY {
                    handle_hotkey(msg.wParam as i32, &cmd_tx);
                }
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            // 托盘状态更新（内部有 200ms 限流）
            update_tray(&tray, &last_sync_at, &uploading, &connected, &status_rx,
                &mut icon_state, &mut last_icon_update,
                &mut menu_dirty, &mut last_sync_time, &mut last_sync_from,
                &mut auto_sync, &cfg, &cmd_tx, &mut last_icon_color, &mut last_tooltip);

            while let Ok(event) = menu_rx.try_recv() {
                handle_menu_event(event.id, &mut cfg, &cmd_tx, &mut auto_sync, &shutdown, &mut menu_dirty);
            }

            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

// ── 热键 ──

fn register_hotkeys(cfg: &config::Config) {
    unsafe {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::RegisterHotKey;
        if let Some((mods, vk)) = parse_hotkey(&cfg.hotkey_copy) {
            RegisterHotKey(std::ptr::null_mut(), 1, mods, vk as u32);
        }
        if let Some((mods, vk)) = parse_hotkey(&cfg.hotkey_paste) {
            RegisterHotKey(std::ptr::null_mut(), 2, mods, vk as u32);
        }
        if let Some((mods, vk)) = parse_hotkey(&cfg.hotkey_toggle) {
            RegisterHotKey(std::ptr::null_mut(), 3, mods, vk as u32);
        }
    }
}

fn parse_hotkey(s: &str) -> Option<(u32, u16)> {
    let parts: Vec<&str> = s.split('+').map(str::trim).collect();
    let key = *parts.last()?;
    let mut mods = 0u32;
    for m in &parts[..parts.len() - 1] {
        match *m {
            "Ctrl" => mods |= 0x0002,
            "Shift" => mods |= 0x0004,
            "Alt" => mods |= 0x0001,
            "Win" => mods |= 0x0008,
            _ => return None,
        }
    }
    let vk = match key.as_bytes() {
        [b] if b.is_ascii_alphabetic() => b.to_ascii_uppercase() as u16,
        [b @ b'0'..=b'9'] => *b as u16,
        _ => return None,
    };
    Some((mods, vk))
}

fn handle_hotkey(id: i32, cmd_tx: &crossbeam_channel::Sender<SyncCommand>) {
    match id {
        1 => { let _ = cmd_tx.send(SyncCommand::CopyAndSync); }
        2 => { let _ = cmd_tx.send(SyncCommand::SyncAndPaste); }
        3 => { let _ = cmd_tx.send(SyncCommand::ToggleAutoSync); }
        _ => {}
    }
}

// ── 托盘菜单 ──

fn en_zh(en: &'static str, zh: &'static str) -> (&'static str, &'static str) { (en, zh) }

fn t<'a>(lang: &str, (en, zh): (&'a str, &'a str)) -> &'a str {
    if lang == "zh" { zh } else { en }
}

fn build_menu(
    _cfg: &config::Config,
    connected: bool,
    last_sync_time: &str,
    last_sync_from: &str,
    auto_sync: bool,
    autostart: bool,
    _cmd_tx: &crossbeam_channel::Sender<SyncCommand>,
    lang: &str,
) -> Menu {
    let menu = Menu::new();

    let (status_text, gr, gg, gb) = if connected {
        (t(lang, en_zh("Connected", "已连接")), 0x44u8, 0xCCu8, 0x44u8)
    } else {
        (t(lang, en_zh("Disconnected", "已断开")), 0xFFu8, 0x44u8, 0x44u8)
    };
    let dot = make_menu_dot(gr, gg, gb);
    let _ = menu.append(&IconMenuItem::with_id("status-text", status_text, true, Some(dot), None));
    let info_dot = make_menu_dot(0x44, 0xAA, 0xFF);
    let _ = menu.append(&IconMenuItem::new(
        format!("{}: {}", t(lang, en_zh("Last Sync", "上次同步")), last_sync_time), true, Some(info_dot.clone()), None,
    ));
    let _ = menu.append(&IconMenuItem::new(
        format!("{}: {}", t(lang, en_zh("From", "来自")), last_sync_from), true, Some(info_dot), None,
    ));
    let _ = menu.append(&PredefinedMenuItem::separator());

    let upload_item = MenuItem::with_id("sync-upload", t(lang, en_zh("Upload", "上传")), true, None);
    let _ = menu.append(&upload_item);
    let download_item = MenuItem::with_id("sync-download", t(lang, en_zh("Download", "下载")), true, None);
    let _ = menu.append(&download_item);

    let toggle_sync = CheckMenuItem::with_id("auto-sync", t(lang, en_zh("Auto-Sync", "自动同步")), true, auto_sync, None);
    let _ = menu.append(&toggle_sync);

    let _ = menu.append(&PredefinedMenuItem::separator());

    let edit_cfg = MenuItem::with_id("edit-config", t(lang, en_zh("Edit Config", "编辑配置")), true, None);
    let open_dir = MenuItem::with_id("open-dir", t(lang, en_zh("Open Config Directory", "打开配置目录")), true, None);
    let lang_name = if lang == "zh" { "English" } else { "中文" };
    let lang_switch = MenuItem::with_id("lang-switch", lang_name, true, None);
    let settings = Submenu::with_items(t(lang, en_zh("Settings", "设置")), true, &[&edit_cfg, &open_dir, &lang_switch])
        .expect("failed to create submenu");
    let _ = menu.append(&settings);

    let autostart_item = CheckMenuItem::with_id("launch-startup", t(lang, en_zh("Launch at Startup", "开机自启")), true, autostart, None);
    let _ = menu.append(&autostart_item);

    let open_log = MenuItem::with_id("open-log", t(lang, en_zh("Open Log", "打开日志")), true, None);
    let _ = menu.append(&open_log);

    let _ = menu.append(&PredefinedMenuItem::separator());
    let restart_item = MenuItem::with_id("tray-restart", t(lang, en_zh("Restart", "重启")), true, None);
    let _ = menu.append(&restart_item);
    let quit_item = MenuItem::with_id("quit", t(lang, en_zh("Quit", "退出")), true, None);
    let _ = menu.append(&quit_item);

    menu
}

fn handle_menu_event(
    id: MenuId,
    cfg: &mut config::Config,
    cmd_tx: &crossbeam_channel::Sender<SyncCommand>,
    auto_sync: &mut bool,
    shutdown: &Arc<AtomicBool>,
    menu_dirty: &mut bool,
) {
    match id.as_ref() {
        "sync-upload" => { let _ = cmd_tx.send(SyncCommand::SyncUpload); }
        "sync-download" => { let _ = cmd_tx.send(SyncCommand::SyncDownload); }
        "auto-sync" => {
            *auto_sync = !*auto_sync;
            let _ = cmd_tx.send(SyncCommand::ToggleAutoSync);
        }
        "edit-config" => {
            if let Ok(path) = config::Config::config_path() {
                let _ = std::process::Command::new("notepad.exe")
                    .arg(&path)
                    .spawn();
            }
        }
        "open-dir" => {
            if let Ok(path) = config::Config::config_path() {
                if let Some(dir) = path.parent() {
                    let _ = std::process::Command::new("explorer.exe")
                        .arg(dir)
                        .spawn();
                }
            }
        }
        "launch-startup" => {
            if let Ok(mut cfg) = config::Config::load() {
                let new_val = !cfg.autostart;
                let _ = cfg.set_autostart(new_val);
            }
        }
        "open-log" => {
            let _ = std::process::Command::new("notepad.exe")
                .arg("clipsync.log")
                .spawn();
        }
        "tray-restart" => {
            match std::env::current_exe() {
                Ok(exe) => {
                    let _ = std::process::Command::new(exe).spawn();
                }
                Err(e) => {
                    tracing::error!("failed to get exe path for restart: {e}");
                }
            }
            std::process::exit(0);
        }
        "quit" => {
            shutdown.store(true, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(100));
            std::process::exit(0);
        }
        "lang-switch" => {
            cfg.language = if cfg.language == "zh" { "en".into() } else { "zh".into() };
            cfg.save().ok();
            *menu_dirty = true;
        }
        _ => {}
    }
}

// ── 动态更新 ──

fn update_tray(
    tray: &tray_icon::TrayIcon,
    last_sync_at: &Arc<AtomicU64>,
    uploading: &Arc<AtomicBool>,
    connected: &Arc<AtomicBool>,
    status_rx: &crossbeam_channel::Receiver<SyncStatus>,
    icon_state: &mut &str,
    last_icon_update: &mut std::time::Instant,
    menu_dirty: &mut bool,
    last_sync_time: &mut String,
    last_sync_from: &mut String,
    auto_sync: &mut bool,
    cfg: &config::Config,
    cmd_tx: &crossbeam_channel::Sender<SyncCommand>,
    last_icon_color: &mut Option<(u8, u8, u8)>,
    last_tooltip: &mut Option<String>,
) {
    while let Ok(status) = status_rx.try_recv() {
        connected.store(status.connected, Ordering::SeqCst);
        if !status.last_sync_time.is_empty() {
            *last_sync_time = status.last_sync_time;
        }
        if !status.last_sync_from.is_empty() {
            *last_sync_from = status.last_sync_from;
        }
        *auto_sync = status.auto_sync;
        *menu_dirty = true;
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let last = last_sync_at.load(Ordering::SeqCst);
    let is_connected = connected.load(Ordering::SeqCst);
    let is_syncing = now.saturating_sub(last) < 800 || uploading.load(Ordering::SeqCst);

    let now = std::time::Instant::now();
    if now.duration_since(*last_icon_update) >= Duration::from_millis(200) {
        *last_icon_update = now;

        let (r, g, b) = if is_syncing {
            if *icon_state == "syncing_on" {
                *icon_state = "syncing_off";
                (0x44, 0xAA, 0xFF)
            } else {
                *icon_state = "syncing_on";
                (0x88, 0xCC, 0xFF)
            }
        } else if is_connected {
            *icon_state = "connected";
            (0x44, 0xCC, 0x44)
        } else {
            *icon_state = "disconnected";
            (0xFF, 0x44, 0x44)
        };
        if *last_icon_color != Some((r, g, b)) {
            *last_icon_color = Some((r, g, b));
            if let Err(e) = tray.set_icon(Some(make_circle_icon(r, g, b))) {
                tracing::warn!("failed to set tray icon: {e}");
            }
        }
    }

    if *menu_dirty {
        let new_menu = build_menu(cfg, is_connected, last_sync_time, last_sync_from,
            *auto_sync, cfg.autostart, cmd_tx, &cfg.language);
        tray.set_menu(Some(Box::new(new_menu)));
        *menu_dirty = false;
    }

    let ts = if is_syncing {
        if cfg.language == "zh" { " 同步中..." } else { " syncing..." }
    } else if is_connected {
        if cfg.language == "zh" { " 已连接" } else { " connected" }
    } else {
        if cfg.language == "zh" { " 已断开" } else { " disconnected" }
    };
    let tooltip = format!("ClipSync -{ts}");
    if *last_tooltip != Some(tooltip.clone()) {
        *last_tooltip = Some(tooltip.clone());
        let _ = tray.set_tooltip(Some(tooltip));
    }
}

// ── 图标 ──

fn make_menu_dot(r: u8, g: u8, b: u8) -> MenuIcon {
    let size = 16u32;
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    let cx = 7.5f32;
    let cy = 7.5f32;
    let radius = 5.5f32;
    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 + 0.5 - cx;
            let dy = y as f32 + 0.5 - cy;
            let dist_sq = dx * dx + dy * dy;
            if dist_sq <= radius * radius {
                let alpha = if dist_sq <= (radius - 0.5) * (radius - 0.5) {
                    255
                } else {
                    ((radius - dist_sq.sqrt() + 0.5) * 255.0) as u8
                };
                rgba.extend_from_slice(&[r, g, b, alpha]);
            } else {
                rgba.extend_from_slice(&[0, 0, 0, 0]);
            }
        }
    }
    MenuIcon::from_rgba(rgba, size, size).expect("failed to create menu icon")
}

fn make_circle_icon(r: u8, g: u8, b: u8) -> Icon {
    let size = 32u32;
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    let cx = 15.5f32;
    let cy = 15.5f32;
    let radius = 14.0f32;
    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 + 0.5 - cx;
            let dy = y as f32 + 0.5 - cy;
            if dx * dx + dy * dy <= radius * radius {
                let idx = ((y * size + x) * 4) as usize;
                rgba[idx] = r;
                rgba[idx + 1] = g;
                rgba[idx + 2] = b;
                rgba[idx + 3] = 255;
            }
        }
    }
    Icon::from_rgba(rgba, size, size).expect("failed to create icon")
}
