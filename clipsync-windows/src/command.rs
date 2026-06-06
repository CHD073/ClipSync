#[derive(Debug, Clone)]
pub enum SyncCommand {
    SyncUpload,
    SyncDownload,
    ToggleAutoSync,
    CopyAndSync,
    SyncAndPaste,
}

#[derive(Debug, Clone)]
pub struct SyncStatus {
    pub connected: bool,
    pub auto_sync: bool,
    pub last_sync_time: String,
    pub last_sync_from: String,
}
