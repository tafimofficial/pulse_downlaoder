mod autostart;
mod downloader;
mod popup;
mod server;
mod storage;
mod types;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Listener, Manager, State, WindowEvent};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::autostart::set_autostart;
use crate::downloader::DownloadManager;
use crate::popup::open_download_popup;
use crate::server::start_extension_server;
use crate::storage::{load_downloads, load_settings, save_downloads, save_settings};
use crate::types::{AppSettings, DownloadItem, DownloadProgressPayload, DownloadStatus};

pub struct AppState {
    pub download_manager: Arc<DownloadManager>,
    pub downloads: Arc<Mutex<Vec<DownloadItem>>>,
    pub settings: Arc<Mutex<AppSettings>>,
}

#[tauri::command]
async fn get_downloads(state: State<'_, AppState>) -> Result<Vec<DownloadItem>, String> {
    let list = state.downloads.lock().await;
    Ok(list.clone())
}

fn generate_unique_filename(target_dir: &Path, base_name: &str, existing_items: &[DownloadItem]) -> String {
    let p = Path::new(base_name);
    let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("Download");
    let ext = p.extension().and_then(|e| e.to_str());

    let mut candidate = base_name.to_string();
    let mut counter = 1;

    loop {
        let dest = target_dir.join(&candidate);
        let dest_str = dest.to_string_lossy().to_string();

        let file_exists = dest.exists();
        let in_list = existing_items.iter().any(|item| {
            item.save_path.eq_ignore_ascii_case(&dest_str)
                && item.status != DownloadStatus::Error
        });

        if !file_exists && !in_list {
            return candidate;
        }

        candidate = match ext {
            Some(e) if !e.is_empty() => format!("{} ({}).{}", stem, counter, e),
            _ => format!("{} ({})", stem, counter),
        };
        counter += 1;
    }
}

#[tauri::command]
async fn add_download(
    url: String,
    filename: Option<String>,
    save_dir: Option<String>,
    is_video: Option<bool>,
    quality: Option<String>,
    connections: Option<u32>,
    cookies: Option<String>,
    referer: Option<String>,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<DownloadItem, String> {
    let settings = state.settings.lock().await.clone();
    let id = Uuid::new_v4().to_string();

    let target_dir = save_dir
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(settings.default_download_dir.clone());

    let is_vid = is_video.unwrap_or(false) || is_video_site_url(&url);

    let mut final_filename = if let Some(fn_val) = filename.filter(|s| !s.trim().is_empty()) {
        sanitize_filename(&fn_val)
    } else {
        extract_filename(&url, is_vid)
    };

    if is_vid {
        let is_audio = quality.as_deref() == Some("audio");
        let ext = if is_audio { ".mp3" } else { ".mp4" };
        let lower = final_filename.to_lowercase();
        if !lower.ends_with(".mp4")
            && !lower.ends_with(".mkv")
            && !lower.ends_with(".webm")
            && !lower.ends_with(".mp3")
            && !lower.ends_with(".m4a")
        {
            final_filename.push_str(ext);
        }
    }

    let unique_filename = {
        let list = state.downloads.lock().await;
        generate_unique_filename(Path::new(&target_dir), &final_filename, &list)
    };

    let save_path = PathBuf::from(&target_dir).join(&unique_filename);
    let save_path_str = save_path.to_string_lossy().to_string();

    let item = DownloadItem {
        id: id.clone(),
        url: url.clone(),
        filename: unique_filename.clone(),
        save_path: save_path_str,
        total_bytes: None,
        downloaded_bytes: 0,
        status: DownloadStatus::Downloading,
        speed_bps: 0,
        eta_seconds: None,
        is_video: is_vid,
        quality,
        connections: connections.unwrap_or(settings.default_connections),
        cookies,
        referer,
        error_msg: None,
        created_at: chrono::Local::now().to_rfc3339(),
    };

    {
        let mut list = state.downloads.lock().await;
        list.insert(0, item.clone());
        save_downloads(&list);
    }

    let _ = app_handle.emit("download-added", item.clone());

    state
        .download_manager
        .start_download(item.clone(), settings.speed_limit_bps)
        .await;

    if settings.show_popup_on_download {
        open_download_popup(&app_handle, &id, &unique_filename);
    }

    Ok(item)
}

#[tauri::command]
async fn pause_download(id: String, state: State<'_, AppState>) -> Result<(), String> {
    state.download_manager.cancel_or_pause(&id).await;
    let mut list = state.downloads.lock().await;
    if let Some(item) = list.iter_mut().find(|i| i.id == id) {
        item.status = DownloadStatus::Paused;
        item.speed_bps = 0;
        item.eta_seconds = None;
        save_downloads(&list);
    }
    Ok(())
}

#[tauri::command]
async fn resume_download(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let settings = state.settings.lock().await.clone();
    let mut list = state.downloads.lock().await;
    if let Some(item) = list.iter_mut().find(|i| i.id == id) {
        item.status = DownloadStatus::Downloading;
        let item_clone = item.clone();
        save_downloads(&list);
        state
            .download_manager
            .start_download(item_clone, settings.speed_limit_bps)
            .await;
    }
    Ok(())
}

#[tauri::command]
async fn cancel_download(
    id: String,
    delete_file: Option<bool>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.download_manager.cancel_or_pause(&id).await;
    let mut list = state.downloads.lock().await;

    if let Some(pos) = list.iter().position(|i| i.id == id) {
        let item = list.remove(pos);
        save_downloads(&list);

        // Always clean up incomplete chunk/part files on cancel so future downloads start fresh
        let _ = std::fs::remove_file(format!("{}.part", item.save_path));
        for i in 0..64 {
            let _ = std::fs::remove_file(format!("{}.part{}", item.save_path, i));
        }

        if delete_file.unwrap_or(true) {
            let _ = std::fs::remove_file(&item.save_path);
        }
    }
    Ok(())
}

#[tauri::command]
async fn batch_pause(ids: Vec<String>, state: State<'_, AppState>) -> Result<(), String> {
    for id in &ids {
        state.download_manager.cancel_or_pause(id).await;
    }
    let mut list = state.downloads.lock().await;
    for item in list.iter_mut() {
        if ids.contains(&item.id) && item.status == DownloadStatus::Downloading {
            item.status = DownloadStatus::Paused;
            item.speed_bps = 0;
            item.eta_seconds = None;
        }
    }
    save_downloads(&list);
    Ok(())
}

#[tauri::command]
async fn batch_resume(ids: Vec<String>, state: State<'_, AppState>) -> Result<(), String> {
    let settings = state.settings.lock().await.clone();
    let mut to_start = Vec::new();

    {
        let mut list = state.downloads.lock().await;
        for item in list.iter_mut() {
            if ids.contains(&item.id) && item.status != DownloadStatus::Completed {
                item.status = DownloadStatus::Downloading;
                to_start.push(item.clone());
            }
        }
        save_downloads(&list);
    }

    for item in to_start {
        state
            .download_manager
            .start_download(item, settings.speed_limit_bps)
            .await;
    }
    Ok(())
}

#[tauri::command]
async fn batch_delete(
    ids: Vec<String>,
    delete_files: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    for id in &ids {
        state.download_manager.cancel_or_pause(id).await;
    }

    let mut list = state.downloads.lock().await;
    let mut removed = Vec::new();

    list.retain(|item| {
        if ids.contains(&item.id) {
            removed.push(item.clone());
            false
        } else {
            true
        }
    });

    save_downloads(&list);

    if delete_files {
        for item in removed {
            let _ = std::fs::remove_file(&item.save_path);
            let _ = std::fs::remove_file(format!("{}.part", item.save_path));
            for i in 0..32 {
                let _ = std::fs::remove_file(format!("{}.part{}", item.save_path, i));
            }
        }
    }
    Ok(())
}

#[tauri::command]
async fn pause_all(state: State<'_, AppState>) -> Result<(), String> {
    let mut list = state.downloads.lock().await;
    for item in list.iter_mut() {
        if item.status == DownloadStatus::Downloading {
            state.download_manager.cancel_or_pause(&item.id).await;
            item.status = DownloadStatus::Paused;
            item.speed_bps = 0;
            item.eta_seconds = None;
        }
    }
    save_downloads(&list);
    Ok(())
}

#[tauri::command]
async fn resume_all(state: State<'_, AppState>) -> Result<(), String> {
    let settings = state.settings.lock().await.clone();
    let mut to_start = Vec::new();
    {
        let mut list = state.downloads.lock().await;
        for item in list.iter_mut() {
            if item.status == DownloadStatus::Paused || item.status == DownloadStatus::Queued {
                item.status = DownloadStatus::Downloading;
                to_start.push(item.clone());
            }
        }
        save_downloads(&list);
    }

    for item in to_start {
        state
            .download_manager
            .start_download(item, settings.speed_limit_bps)
            .await;
    }
    Ok(())
}

#[tauri::command]
async fn clear_completed(state: State<'_, AppState>) -> Result<(), String> {
    let mut list = state.downloads.lock().await;
    list.retain(|i| i.status != DownloadStatus::Completed);
    save_downloads(&list);
    Ok(())
}

#[tauri::command]
async fn get_settings(state: State<'_, AppState>) -> Result<AppSettings, String> {
    let s = state.settings.lock().await;
    Ok(s.clone())
}

#[tauri::command]
async fn update_settings(
    settings: AppSettings,
    state: State<'_, AppState>,
) -> Result<(), String> {
    set_autostart(settings.autostart);
    save_settings(&settings);
    let mut s = state.settings.lock().await;
    *s = settings;
    Ok(())
}

#[tauri::command]
fn open_file(path: String) -> Result<(), String> {
    std::process::Command::new("explorer")
        .arg(&path)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn open_folder(path: String) -> Result<(), String> {
    let p = PathBuf::from(&path);
    let target_folder = if p.is_dir() {
        p
    } else if let Some(parent) = p.parent() {
        if parent.as_os_str().is_empty() {
            dirs::download_dir().unwrap_or_else(|| PathBuf::from("C:\\Downloads"))
        } else {
            parent.to_path_buf()
        }
    } else {
        dirs::download_dir().unwrap_or_else(|| PathBuf::from("C:\\Downloads"))
    };

    if target_folder.exists() {
        std::process::Command::new("explorer")
            .arg(&target_folder)
            .spawn()
            .map_err(|e| e.to_string())?;
    } else if let Some(dl) = dirs::download_dir() {
        let _ = std::process::Command::new("explorer").arg(dl).spawn();
    }
    Ok(())
}

#[tauri::command]
async fn close_popup_window(id: String, app_handle: AppHandle) -> Result<(), String> {
    let window_label = format!("popup_{}", id);
    if let Some(win) = app_handle.get_webview_window(&window_label) {
        let _ = win.destroy();
    }
    Ok(())
}

#[tauri::command]
fn pick_folder() -> Option<String> {
    let mut cmd = std::process::Command::new("powershell");
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    let output = cmd
        .args([
            "-NoProfile",
            "-Command",
            "[System.Reflection.Assembly]::LoadWithPartialName('System.windows.forms') | Out-Null; $f = New-Object System.Windows.Forms.FolderBrowserDialog; $f.Description = 'Select PulseDL Download Folder'; if ($f.ShowDialog() -eq 'OK') { Write-Output $f.SelectedPath }",
        ])
        .output()
        .ok()?;

    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path.is_empty() {
        None
    } else {
        Some(path)
    }
}

#[tauri::command]
async fn open_popup_window(
    id: String,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let list = state.downloads.lock().await;
    if let Some(item) = list.iter().find(|i| i.id == id) {
        open_download_popup(&app_handle, &item.id, &item.filename);
    }
    Ok(())
}

pub fn sanitize_filename(name: &str) -> String {
    let invalid_chars = ['\\', '/', ':', '*', '?', '"', '<', '>', '|'];
    let mut clean: String = name
        .chars()
        .map(|c| if invalid_chars.contains(&c) || c.is_control() { '_' } else { c })
        .collect();
    clean = clean.trim().trim_matches('.').to_string();
    if clean.is_empty() {
        return format!("Video_{}.mp4", chrono::Local::now().format("%Y%m%d_%H%M%S"));
    }
    if clean.len() > 180 {
        clean.truncate(180);
    }
    clean
}

fn extract_filename(url_str: &str, is_video: bool) -> String {
    if is_video {
        return format!("Video_{}.mp4", chrono::Local::now().format("%Y%m%d_%H%M%S"));
    }

    if let Ok(parsed) = url::Url::parse(url_str) {
        if let Some(segments) = parsed.path_segments() {
            if let Some(last) = segments.last() {
                let decoded = urlencoding_decode(last);
                let cleaned = sanitize_filename(&decoded);
                if !cleaned.trim().is_empty() && cleaned.contains('.') {
                    return cleaned;
                }
            }
        }
    }
    format!("Download_{}.bin", chrono::Local::now().format("%Y%m%d_%H%M%S"))
}

fn urlencoding_decode(s: &str) -> String {
    url::form_urlencoded::parse(s.as_bytes())
        .map(|(k, _)| k.to_string())
        .next()
        .unwrap_or_else(|| s.to_string())
}

fn is_video_site_url(url: &str) -> bool {
    let u = url.to_lowercase();
    u.contains("youtube.com")
        || u.contains("youtu.be")
        || u.contains("facebook.com")
        || u.contains("fb.watch")
        || u.contains("tiktok.com")
        || u.contains("instagram.com")
        || u.contains("vimeo.com")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let initial_downloads = load_downloads();
    let initial_settings = load_settings();

    // Check if Windows startup with --minimized
    let start_minimized = std::env::args().any(|arg| arg == "--minimized" || arg == "--tray");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(move |app| {
            let app_handle = app.handle().clone();

            let download_manager = Arc::new(DownloadManager::new(app_handle.clone()));
            let downloads_state = Arc::new(Mutex::new(initial_downloads));
            let settings_state = Arc::new(Mutex::new(initial_settings.clone()));

            // Register state
            app.manage(AppState {
                download_manager: Arc::clone(&download_manager),
                downloads: Arc::clone(&downloads_state),
                settings: Arc::clone(&settings_state),
            });

            // Start local extension HTTP server
            start_extension_server(app_handle.clone());

            // Build Tray Icon
            let quit_i = MenuItem::with_id(app, "quit", "Quit PulseDL", true, None::<&str>)?;
            let show_i = MenuItem::with_id(app, "show", "Open PulseDL", true, None::<&str>)?;
            let pause_all_i = MenuItem::with_id(app, "pause_all", "Pause All Downloads", true, None::<&str>)?;
            let resume_all_i = MenuItem::with_id(app, "resume_all", "Resume All Downloads", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_i, &pause_all_i, &resume_all_i, &quit_i])?;

            if let Some(icon) = app.default_window_icon().cloned() {
                let _tray = TrayIconBuilder::new()
                    .menu(&menu)
                    .icon(icon)
                    .tooltip("PulseDL Turbo Downloader (99% Bandwidth Active)")
                    .on_menu_event(|app, event| match event.id.as_ref() {
                        "quit" => {
                            app.exit(0);
                        }
                        "show" => {
                            if let Some(win) = app.get_webview_window("main") {
                                let _ = win.unminimize();
                                let _ = win.show();
                                let _ = win.set_focus();
                            }
                        }
                        "pause_all" => {
                            let _ = app.emit("tray-pause-all", ());
                        }
                        "resume_all" => {
                            let _ = app.emit("tray-resume-all", ());
                        }
                        _ => {}
                    })
                    .on_tray_icon_event(|tray, event| {
                        if let TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        } = event
                        {
                            let app = tray.app_handle();
                            if let Some(win) = app.get_webview_window("main") {
                                let _ = win.unminimize();
                                let _ = win.show();
                                let _ = win.set_focus();
                            }
                        }
                    })
                    .build(app)?;
            }

            // Ensure main window is created and handle close-to-tray
            let main_win = match app.get_webview_window("main") {
                Some(w) => w,
                None => {
                    tauri::WebviewWindowBuilder::new(app, "main", tauri::WebviewUrl::App("index.html".into()))
                        .title("PulseDL - Ultra Fast Turbo Downloader")
                        .inner_size(1080.0, 700.0)
                        .min_inner_size(840.0, 540.0)
                        .center()
                        .decorations(true)
                        .visible(!start_minimized)
                        .build()?
                }
            };

            if start_minimized {
                let _ = main_win.hide();
            } else {
                let _ = main_win.unminimize();
                let _ = main_win.show();
                let _ = main_win.set_focus();
            }

            let win_clone = main_win.clone();
            main_win.on_window_event(move |event| {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = win_clone.hide();
                }
            });

            // Progress event listener to update downloads state & persist
            let dl_state_clone = Arc::clone(&downloads_state);
            let app_h = app_handle.clone();
            app.listen("download-progress", move |event| {
                if let Ok(progress) = serde_json::from_str::<DownloadProgressPayload>(event.payload()) {
                    let dl_mutex = Arc::clone(&dl_state_clone);
                    let app_sub = app_h.clone();
                    tauri::async_runtime::spawn(async move {
                        let mut list = dl_mutex.lock().await;
                        let mut completed_id = None;
                        if let Some(item) = list.iter_mut().find(|i| i.id == progress.id) {
                            item.downloaded_bytes = progress.downloaded_bytes;
                            if let Some(tot) = progress.total_bytes {
                                item.total_bytes = Some(tot);
                            }
                            item.speed_bps = progress.speed_bps;
                            item.eta_seconds = progress.eta_seconds;
                            item.status = progress.status;
                            if progress.error_msg.is_some() {
                                item.error_msg = progress.error_msg;
                            }

                            if item.status == DownloadStatus::Completed {
                                completed_id = Some(item.id.clone());
                            }
                        }

                        if let Some(cid) = completed_id {
                            save_downloads(&list);
                            let _ = app_sub.emit("download-completed", cid);
                        }
                    });
                }
            });

            // Handle incoming downloads from Chrome/Edge extension
            let app_ext = app_handle.clone();
            app.listen("extension-add-download", move |event| {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(event.payload()) {
                    let app_h = app_ext.clone();
                    tauri::async_runtime::spawn(async move {
                        let url = val.get("url").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let filename = val.get("filename").and_then(|v| v.as_str()).map(|s| s.to_string());
                        let quality = val.get("quality").and_then(|v| v.as_str()).map(|s| s.to_string());
                        let is_video = val.get("is_video").and_then(|v| v.as_bool());
                        let cookies = val.get("cookies").and_then(|v| v.as_str()).map(|s| s.to_string());
                        let referer = val.get("referer").and_then(|v| v.as_str()).map(|s| s.to_string());

                        if !url.is_empty() {
                            let state: State<'_, AppState> = app_h.state();
                            let _ = add_download(
                                url,
                                filename,
                                None,
                                is_video,
                                quality,
                                None,
                                cookies,
                                referer,
                                app_h.clone(),
                                state,
                            ).await;
                        }
                    });
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_downloads,
            add_download,
            pause_download,
            resume_download,
            cancel_download,
            batch_pause,
            batch_resume,
            batch_delete,
            pause_all,
            resume_all,
            clear_completed,
            get_settings,
            update_settings,
            open_file,
            open_folder,
            pick_folder,
            open_popup_window,
            close_popup_window
        ])
        .run(tauri::generate_context!())
        .expect("error while running PulseDL application");
}
