use std::path::PathBuf;

use anyhow::Context;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

fn default_true() -> bool { true }
fn default_auto_sync_max_bytes() -> usize { 10_485_760 }
fn default_http_timeout() -> u64 { 180 }
fn default_hotkey_copy() -> String { "Ctrl+Shift+C".into() }
fn default_hotkey_paste() -> String { "Ctrl+Shift+V".into() }
fn default_hotkey_toggle() -> String { "Ctrl+Alt+V".into() }
fn default_language() -> String { "en".into() }

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub server_url: String,
    pub token: String,
    pub device_id: String,
    pub device_name: String,
    pub ws_inline_max_bytes: usize,
    #[serde(default = "default_auto_sync_max_bytes")]
    pub auto_sync_max_bytes: usize,
    #[serde(default = "default_http_timeout")]
    pub http_timeout_secs: u64,
    #[serde(default = "default_true")]
    pub auto_sync: bool,
    #[serde(default)]
    pub autostart: bool,
    #[serde(default = "default_hotkey_copy")]
    pub hotkey_copy: String,
    #[serde(default = "default_hotkey_paste")]
    pub hotkey_paste: String,
    #[serde(default = "default_hotkey_toggle")]
    pub hotkey_toggle: String,
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(skip)]
    pub config_path: PathBuf,
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        let config_path = Self::path()?;
        let mut cfg: Config = if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)
                .context("failed to read config file")?;
            toml::from_str(&content)
                .context("failed to parse config file")?
        } else {
            let mut c = Self::default();
            c.config_path = config_path.clone();
            Self::ensure_device_id(&mut c);
            c.save()?;
            return Ok(c);
        };
        cfg.config_path = config_path;
        Self::ensure_device_id(&mut cfg);
        cfg.save()?;
        Ok(cfg)
    }

    pub fn config_path() -> anyhow::Result<PathBuf> { Self::path() }

    fn path() -> anyhow::Result<PathBuf> {
        let exe = std::env::current_exe()
            .context("failed to get executable path")?;
        let dir = exe.parent()
            .context("failed to get executable directory")?;
        Ok(dir.join("config.toml"))
    }

    fn ensure_device_id(cfg: &mut Config) {
        if cfg.device_id.is_empty() {
            cfg.device_id = Uuid::new_v4().to_string();
            cfg.save().ok();
        }
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let content = toml::to_string_pretty(self)?;
        std::fs::write(&self.config_path, content)?;
        Ok(())
    }

    pub fn set_autostart(&mut self, enabled: bool) -> anyhow::Result<()> {
        #[cfg(windows)]
        {
            use std::ffi::OsStr;
            use std::os::windows::ffi::OsStrExt;
            use windows_sys::Win32::System::Registry::*;

            let key = "Software\\Microsoft\\Windows\\CurrentVersion\\Run\0";
            let key_wide: Vec<u16> = OsStr::new(key).encode_wide().collect();
            let mut hkey: HKEY = std::ptr::null_mut();

            let status = unsafe {
                RegOpenKeyExW(
                    HKEY_CURRENT_USER,
                    key_wide.as_ptr(),
                    0,
                    KEY_SET_VALUE,
                    &mut hkey,
                )
            };
            if status != 0 {
                anyhow::bail!("failed to open registry key: {status}");
            }

            if enabled {
                let exe_path = std::env::current_exe()
                    .context("failed to get executable path")?;
                let exe_str = exe_path.to_string_lossy();
                let exe_wide: Vec<u16> = OsStr::new(&*exe_str).encode_wide().collect();
                let name = "LiteClipSync\0";
                let name_wide: Vec<u16> = OsStr::new(name).encode_wide().collect();
                unsafe {
                    RegSetValueExW(
                        hkey,
                        name_wide.as_ptr(),
                        0,
                        REG_SZ,
                        exe_wide.as_ptr() as *const u8,
                        (exe_wide.len() * 2) as u32,
                    );
                }
            } else {
                let name = "LiteClipSync\0";
                let name_wide: Vec<u16> = OsStr::new(name).encode_wide().collect();
                unsafe {
                    RegDeleteValueW(hkey, name_wide.as_ptr());
                }
            }

            unsafe { RegCloseKey(hkey); }
            self.autostart = enabled;
            self.save()?;
        }
        Ok(())
    }

    fn default() -> Self {
        let hostname = std::env::var("COMPUTERNAME")
            .or_else(|_| std::env::var("HOSTNAME"))
            .unwrap_or_else(|_| "windows-pc".into());
        Self {
            server_url: "http://192.168.245.134:8765".into(),
            token: "liteclipsync".into(),
            device_id: String::new(),
            device_name: hostname,
            ws_inline_max_bytes: 1_048_576,
            auto_sync_max_bytes: 10_485_760,
            http_timeout_secs: 180,
            auto_sync: true,
            autostart: false,
            hotkey_copy: "Ctrl+Shift+C".into(),
            hotkey_paste: "Ctrl+Shift+V".into(),
            hotkey_toggle: "Ctrl+Alt+V".into(),
            language: "en".into(),
            config_path: PathBuf::new(),
        }
    }

    pub fn ws_url(&self) -> String {
        self.server_url
            .replace("http://", "ws://")
            .replace("https://", "wss://")
            + "/ws"
    }

    pub fn api_base(&self) -> String {
        self.server_url.trim_end_matches('/').to_string()
    }

    pub fn basic_auth_value(&self) -> String {
        let encoded = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            format!("{}:", self.token),
        );
        format!("Basic {}", encoded)
    }
}
