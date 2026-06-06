use std::path::PathBuf;

#[derive(Clone)]
pub struct Config {
    pub port: u16,
    pub token: String,
    pub storage_path: PathBuf,
    pub max_history_days: i64,
    pub ws_inline_max_bytes: usize,
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        Ok(Self {
            port: std::env::var("PORT").ok().and_then(|v| v.parse().ok()).unwrap_or(8765),
            token: std::env::var("SYNC_TOKEN").unwrap_or_else(|_| "clipsync".into()),
            storage_path: std::env::var("STORAGE_PATH")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("./data")),
            max_history_days: std::env::var("MAX_HISTORY_DAYS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(7),
            ws_inline_max_bytes: 1_048_576,
        })
    }

    pub fn token(&self) -> &str {
        &self.token
    }
}
