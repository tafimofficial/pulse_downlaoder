use std::fs::{self, File};
use std::io::{BufReader, BufWriter};
use std::path::PathBuf;
use crate::types::{AppSettings, DownloadItem, DownloadStatus};

fn get_app_dir() -> PathBuf {
    let mut dir = dirs::data_dir().unwrap_or_else(|| PathBuf::from("C:\\ProgramData"));
    dir.push("PulseDL");
    if !dir.exists() {
        let _ = fs::create_dir_all(&dir);
    }
    dir
}

pub fn load_downloads() -> Vec<DownloadItem> {
    let path = get_app_dir().join("downloads.json");
    if !path.exists() {
        return Vec::new();
    }

    if let Ok(file) = File::open(path) {
        let reader = BufReader::new(file);
        if let Ok(mut items) = serde_json::from_reader::<_, Vec<DownloadItem>>(reader) {
            // If any were in "Downloading" or "Merging" when PC closed/restarted, set them to Paused
            for item in &mut items {
                if item.status == DownloadStatus::Downloading || item.status == DownloadStatus::Merging {
                    item.status = DownloadStatus::Paused;
                    item.speed_bps = 0;
                    item.eta_seconds = None;
                }
            }
            return items;
        }
    }
    Vec::new()
}

pub fn save_downloads(items: &[DownloadItem]) {
    let dir = get_app_dir();
    let path = dir.join("downloads.json");
    let tmp_path = dir.join("downloads.json.tmp");

    if let Ok(file) = File::create(&tmp_path) {
        let writer = BufWriter::new(file);
        if serde_json::to_writer_pretty(writer, items).is_ok() {
            let _ = fs::rename(tmp_path, path);
        }
    }
}

pub fn load_settings() -> AppSettings {
    let path = get_app_dir().join("settings.json");
    if !path.exists() {
        return AppSettings::default();
    }

    if let Ok(file) = File::open(path) {
        let reader = BufReader::new(file);
        if let Ok(settings) = serde_json::from_reader::<_, AppSettings>(reader) {
            return settings;
        }
    }
    AppSettings::default()
}

pub fn save_settings(settings: &AppSettings) {
    let dir = get_app_dir();
    let path = dir.join("settings.json");
    let tmp_path = dir.join("settings.json.tmp");

    if let Ok(file) = File::create(&tmp_path) {
        let writer = BufWriter::new(file);
        if serde_json::to_writer_pretty(writer, settings).is_ok() {
            let _ = fs::rename(tmp_path, path);
        }
    }
}
