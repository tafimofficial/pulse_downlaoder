use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DownloadStatus {
    Queued,
    Downloading,
    Paused,
    Merging,
    Completed,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadItem {
    pub id: String,
    pub url: String,
    pub filename: String,
    pub save_path: String,
    pub total_bytes: Option<u64>,
    pub downloaded_bytes: u64,
    pub status: DownloadStatus,
    pub speed_bps: u64,
    pub eta_seconds: Option<u64>,
    pub is_video: bool,
    pub quality: Option<String>,
    pub connections: u32,
    pub cookies: Option<String>,
    pub referer: Option<String>,
    pub error_msg: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadProgressPayload {
    pub id: String,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub speed_bps: u64,
    pub eta_seconds: Option<u64>,
    pub status: DownloadStatus,
    pub error_msg: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub default_download_dir: String,
    pub speed_limit_bps: u64, // 0 = unlimited 99% bandwidth
    pub autostart: bool,
    pub minimize_to_tray: bool,
    pub show_popup_on_download: bool,
    pub max_active_downloads: usize,
    pub default_connections: u32,
}

impl Default for AppSettings {
    fn default() -> Self {
        let default_dir = dirs::download_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "C:\\Downloads".to_string());

        Self {
            default_download_dir: default_dir,
            speed_limit_bps: 0,
            autostart: true,
            minimize_to_tray: true,
            show_popup_on_download: true,
            max_active_downloads: 5,
            default_connections: 32,
        }
    }
}
