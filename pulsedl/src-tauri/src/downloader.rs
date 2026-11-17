use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use reqwest::header::{ACCEPT_RANGES, CONTENT_DISPOSITION, CONTENT_LENGTH, COOKIE, RANGE, REFERER, USER_AGENT};
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncBufReadExt;
use tokio::process::Command as TokioCommand;
use tokio::sync::Mutex;

use crate::types::{DownloadItem, DownloadProgressPayload, DownloadStatus};

const BROWSER_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36";

pub struct DownloadManager {
    app_handle: AppHandle,
    active_cancel_tokens: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
}

impl DownloadManager {
    pub fn new(app_handle: AppHandle) -> Self {
        Self {
            app_handle,
            active_cancel_tokens: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn cancel_or_pause(&self, id: &str) {
        let mut tokens = self.active_cancel_tokens.lock().await;
        if let Some(token) = tokens.remove(id) {
            token.store(true, Ordering::SeqCst);
        }
    }

    pub async fn start_download(
        &self,
        item: DownloadItem,
        speed_limit_bps: u64,
    ) {
        let cancel_token = Arc::new(AtomicBool::new(false));
        {
            let mut tokens = self.active_cancel_tokens.lock().await;
            tokens.insert(item.id.clone(), Arc::clone(&cancel_token));
        }

        let app_handle = self.app_handle.clone();
        let tokens_map = Arc::clone(&self.active_cancel_tokens);

        tauri::async_runtime::spawn(async move {
            let id = item.id.clone();
            let is_video = item.is_video || Self::is_video_site(&item.url);

            if is_video {
                Self::download_video_stream(app_handle.clone(), item, cancel_token).await;
            } else {
                Self::download_file_direct(app_handle.clone(), item, speed_limit_bps, cancel_token).await;
            }

            let mut tokens = tokens_map.lock().await;
            tokens.remove(&id);
        });
    }

    fn is_video_site(url: &str) -> bool {
        let u = url.to_lowercase();
        u.contains("youtube.com")
            || u.contains("youtu.be")
            || u.contains("facebook.com")
            || u.contains("fb.watch")
            || u.contains("tiktok.com")
            || u.contains("instagram.com")
            || u.contains("vimeo.com")
            || u.contains("twitter.com")
            || u.contains("x.com")
    }

    async fn download_file_direct(
        app_handle: AppHandle,
        mut item: DownloadItem,
        speed_limit_bps: u64,
        cancel_token: Arc<AtomicBool>,
    ) {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .tcp_nodelay(true)
            .tcp_keepalive(Duration::from_secs(30))
            .pool_max_idle_per_host(64)
            .pool_idle_timeout(Duration::from_secs(60))
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()
            .unwrap_or_default();

        let mut head_req = client.head(&item.url).header(USER_AGENT, BROWSER_USER_AGENT);
        if let Some(ref c) = item.cookies {
            head_req = head_req.header(COOKIE, c);
        }
        if let Some(ref r) = item.referer {
            head_req = head_req.header(REFERER, r);
        }

        let head_res = head_req.send().await;

        let mut total_bytes = None;
        let mut supports_ranges = false;

        if let Ok(ref res) = head_res {
            if let Some(cl) = res.headers().get(CONTENT_LENGTH) {
                if let Ok(cl_str) = cl.to_str() {
                    total_bytes = cl_str.parse::<u64>().ok();
                }
            }
            if let Some(ar) = res.headers().get(ACCEPT_RANGES) {
                if let Ok(ar_str) = ar.to_str() {
                    supports_ranges = ar_str.eq_ignore_ascii_case("bytes");
                }
            }

            // Extract real filename from Content-Disposition header if present (e.g. Google Docs/Drive/ChatGPT exports)
            if let Some(cd) = res.headers().get(CONTENT_DISPOSITION) {
                if let Ok(cd_str) = cd.to_str() {
                    if let Some(extracted_name) = Self::parse_content_disposition_filename(cd_str) {
                        let cur_dest = PathBuf::from(&item.save_path);
                        if let Some(parent) = cur_dest.parent() {
                            let new_save_path = parent.join(&extracted_name);
                            item.filename = extracted_name;
                            item.save_path = new_save_path.to_string_lossy().to_string();
                        }
                    }
                }
            }
        }

        // Strictly verify range support by probing Range: bytes=0-0 to prevent 200 OK duplicate byte loops
        if supports_ranges && total_bytes.is_some() {
            let mut test_req = client
                .get(&item.url)
                .header(USER_AGENT, BROWSER_USER_AGENT)
                .header(RANGE, "bytes=0-0");
            if let Some(ref c) = item.cookies {
                test_req = test_req.header(COOKIE, c);
            }
            if let Some(ref r) = item.referer {
                test_req = test_req.header(REFERER, r);
            }
            if let Ok(probe_res) = test_req.send().await {
                if probe_res.status() != reqwest::StatusCode::PARTIAL_CONTENT {
                    supports_ranges = false;
                }
            } else {
                supports_ranges = false;
            }
        }

        // Emit initial progress
        let _ = app_handle.emit(
            "download-progress",
            DownloadProgressPayload {
                id: item.id.clone(),
                downloaded_bytes: item.downloaded_bytes,
                total_bytes,
                speed_bps: 0,
                eta_seconds: None,
                status: DownloadStatus::Downloading,
                error_msg: None,
            },
        );

        let dest_path = PathBuf::from(&item.save_path);
        if let Some(parent) = dest_path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        let num_parts = if supports_ranges && total_bytes.is_some() && total_bytes.unwrap() > 2 * 1024 * 1024 {
            item.connections.clamp(2, 32) as usize
        } else {
            1
        };

        if num_parts > 1 && total_bytes.is_some() {
            Self::download_multi_chunk(
                app_handle,
                item,
                total_bytes.unwrap(),
                num_parts,
                speed_limit_bps,
                cancel_token,
            )
            .await;
        } else {
            Self::download_single_stream(
                app_handle,
                item,
                total_bytes,
                speed_limit_bps,
                cancel_token,
            )
            .await;
        }
    }

    fn parse_content_disposition_filename(cd_str: &str) -> Option<String> {
        // e.g. attachment; filename="report.docx" or filename*=UTF-8''report.docx
        for part in cd_str.split(';') {
            let part = part.trim();
            if part.to_lowercase().starts_with("filename=") {
                let name = part["filename=".len()..].trim().trim_matches('"').trim_matches('\'');
                if !name.is_empty() {
                    return Some(name.to_string());
                }
            } else if part.to_lowercase().starts_with("filename*=") {
                let val = part["filename*=".len()..].trim();
                if let Some(idx) = val.find("''") {
                    let decoded = val[idx + 2..].trim();
                    return Some(decoded.to_string());
                }
            }
        }
        None
    }

    async fn download_multi_chunk(
        app_handle: AppHandle,
        item: DownloadItem,
        total_size: u64,
        num_parts: usize,
        _speed_limit_bps: u64,
        cancel_token: Arc<AtomicBool>,
    ) {
        let chunk_size = total_size / (num_parts as u64);
        let downloaded_counter = Arc::new(AtomicU64::new(0));

        let mut initial_downloaded = 0u64;
        let mut ranges = Vec::new();

        for i in 0..num_parts {
            let start = i as u64 * chunk_size;
            let end = if i == num_parts - 1 {
                total_size - 1
            } else {
                (i as u64 + 1) * chunk_size - 1
            };

            let part_path = format!("{}.part{}", item.save_path, i);
            let existing_len = fs::metadata(&part_path).map(|m| m.len()).unwrap_or(0);
            let chunk_expected_len = end.saturating_sub(start) + 1;

            // Only reuse existing bytes if within valid chunk range, else clean stale data
            let valid_len = if existing_len <= chunk_expected_len {
                existing_len
            } else {
                let _ = fs::remove_file(&part_path);
                0
            };

            initial_downloaded += valid_len;
            let current_start = start + valid_len;
            ranges.push((i, current_start, end, part_path));
        }

        downloaded_counter.store(initial_downloaded, Ordering::SeqCst);

        let mut tasks = Vec::new();
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .tcp_nodelay(true)
            .tcp_keepalive(Duration::from_secs(30))
            .pool_max_idle_per_host(64)
            .pool_idle_timeout(Duration::from_secs(60))
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()
            .unwrap_or_default();

        for (_part_idx, cur_start, end, part_file_path) in ranges {
            if cur_start > end {
                continue; // Already finished
            }

            let client = client.clone();
            let url = item.url.clone();
            let cookies = item.cookies.clone();
            let referer = item.referer.clone();
            let downloaded_counter = Arc::clone(&downloaded_counter);
            let cancel_token = Arc::clone(&cancel_token);

            tasks.push(tokio::spawn(async move {
                if cancel_token.load(Ordering::SeqCst) {
                    return Ok::<(), String>(());
                }

                let file = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&part_file_path)
                    .map_err(|e| e.to_string())?;

                // 512KB buffered writer for massive throughput & zero Windows disk I/O lock
                let mut writer = std::io::BufWriter::with_capacity(512 * 1024, file);

                let range_hdr = format!("bytes={}-{}", cur_start, end);
                let mut req = client
                    .get(&url)
                    .header(RANGE, range_hdr)
                    .header(USER_AGENT, BROWSER_USER_AGENT);

                if let Some(ref c) = cookies {
                    req = req.header(COOKIE, c);
                }
                if let Some(ref r) = referer {
                    req = req.header(REFERER, r);
                }

                let response = req.send().await.map_err(|e| e.to_string())?;

                // If server does not return 206 Partial Content, abort chunk worker to avoid duplicate download
                if response.status() != reqwest::StatusCode::PARTIAL_CONTENT {
                    return Err(format!("Server returned {} instead of 206 Partial Content", response.status()));
                }

                let mut stream = response.bytes_stream();
                while let Some(chunk_result) = stream.next().await {
                    if cancel_token.load(Ordering::SeqCst) {
                        break;
                    }
                    let chunk = chunk_result.map_err(|e| e.to_string())?;
                    writer.write_all(&chunk).map_err(|e| e.to_string())?;
                    downloaded_counter.fetch_add(chunk.len() as u64, Ordering::SeqCst);
                }

                let _ = writer.flush();
                Ok(())
            }));
        }

        // Monitoring task for progress and speed
        let mon_app_handle = app_handle.clone();
        let mon_id = item.id.clone();
        let mon_counter = Arc::clone(&downloaded_counter);
        let mon_cancel = Arc::clone(&cancel_token);

        let monitor_task = tokio::spawn(async move {
            let mut last_bytes = mon_counter.load(Ordering::Relaxed);
            let mut last_time = Instant::now();

            loop {
                tokio::time::sleep(Duration::from_millis(500)).await;
                if mon_cancel.load(Ordering::SeqCst) {
                    break;
                }

                let cur_bytes = mon_counter.load(Ordering::Relaxed);
                let now = Instant::now();
                let elapsed = now.duration_since(last_time).as_secs_f64();

                let delta = if cur_bytes >= last_bytes {
                    cur_bytes - last_bytes
                } else {
                    0
                };

                let speed_bps = if elapsed > 0.0 {
                    (delta as f64 / elapsed) as u64
                } else {
                    0
                };

                // Clamp cur_bytes to total_size so progress NEVER exceeds 100%
                let safe_bytes = cur_bytes.min(total_size);
                let remaining = total_size.saturating_sub(safe_bytes);
                let eta = if speed_bps > 0 {
                    Some(remaining / speed_bps)
                } else {
                    None
                };

                last_bytes = cur_bytes;
                last_time = now;

                let _ = mon_app_handle.emit(
                    "download-progress",
                    DownloadProgressPayload {
                        id: mon_id.clone(),
                        downloaded_bytes: safe_bytes,
                        total_bytes: Some(total_size),
                        speed_bps,
                        eta_seconds: eta,
                        status: DownloadStatus::Downloading,
                        error_msg: None,
                    },
                );

                if cur_bytes >= total_size {
                    break;
                }
            }
        });

        for t in tasks {
            let _ = t.await;
        }
        let _ = monitor_task.await;

        if cancel_token.load(Ordering::SeqCst) {
            let cur_bytes = downloaded_counter.load(Ordering::SeqCst);
            let _ = app_handle.emit(
                "download-progress",
                DownloadProgressPayload {
                    id: item.id,
                    downloaded_bytes: cur_bytes,
                    total_bytes: Some(total_size),
                    speed_bps: 0,
                    eta_seconds: None,
                    status: DownloadStatus::Paused,
                    error_msg: None,
                },
            );
            return;
        }

        // Merge parts into final destination file
        let _ = app_handle.emit(
            "download-progress",
            DownloadProgressPayload {
                id: item.id.clone(),
                downloaded_bytes: total_size,
                total_bytes: Some(total_size),
                speed_bps: 0,
                eta_seconds: None,
                status: DownloadStatus::Merging,
                error_msg: None,
            },
        );

        let final_dest = PathBuf::from(&item.save_path);
        let final_file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&final_dest);

        if let Ok(mut dest) = final_file {
            let mut buf = vec![0u8; 1024 * 1024]; // 1MB buffer for fast joining
            let mut merge_ok = true;

            for i in 0..num_parts {
                let part_path = format!("{}.part{}", item.save_path, i);
                if let Ok(mut part) = File::open(&part_path) {
                    while let Ok(n) = part.read(&mut buf) {
                        if n == 0 {
                            break;
                        }
                        if dest.write_all(&buf[..n]).is_err() {
                            merge_ok = false;
                            break;
                        }
                    }
                }
                let _ = fs::remove_file(&part_path);
            }

            if merge_ok {
                let _ = app_handle.emit(
                    "download-progress",
                    DownloadProgressPayload {
                        id: item.id,
                        downloaded_bytes: total_size,
                        total_bytes: Some(total_size),
                        speed_bps: 0,
                        eta_seconds: Some(0),
                        status: DownloadStatus::Completed,
                        error_msg: None,
                    },
                );
                return;
            }
        }

        let _ = app_handle.emit(
            "download-progress",
            DownloadProgressPayload {
                id: item.id,
                downloaded_bytes: downloaded_counter.load(Ordering::SeqCst),
                total_bytes: Some(total_size),
                speed_bps: 0,
                eta_seconds: None,
                status: DownloadStatus::Error,
                error_msg: Some("Failed to assemble parts".to_string()),
            },
        );
    }

    async fn download_single_stream(
        app_handle: AppHandle,
        mut item: DownloadItem,
        mut total_bytes: Option<u64>,
        _speed_limit_bps: u64,
        cancel_token: Arc<AtomicBool>,
    ) {
        let part_path = format!("{}.part", item.save_path);
        let mut existing_bytes = fs::metadata(&part_path).map(|m| m.len()).unwrap_or(0);

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()
            .unwrap_or_default();

        let mut req = client.get(&item.url).header(USER_AGENT, BROWSER_USER_AGENT);
        if existing_bytes > 0 {
            req = req.header(RANGE, format!("bytes={}-", existing_bytes));
        }
        if let Some(ref c) = item.cookies {
            req = req.header(COOKIE, c);
        }
        if let Some(ref r) = item.referer {
            req = req.header(REFERER, r);
        }

        let response = match req.send().await {
            Ok(res) => res,
            Err(e) => {
                let _ = app_handle.emit(
                    "download-progress",
                    DownloadProgressPayload {
                        id: item.id,
                        downloaded_bytes: existing_bytes,
                        total_bytes,
                        speed_bps: 0,
                        eta_seconds: None,
                        status: DownloadStatus::Error,
                        error_msg: Some(e.to_string()),
                    },
                );
                return;
            }
        };

        // Check Content-Disposition from response
        if let Some(cd) = response.headers().get(CONTENT_DISPOSITION) {
            if let Ok(cd_str) = cd.to_str() {
                if let Some(extracted_name) = Self::parse_content_disposition_filename(cd_str) {
                    let cur_dest = PathBuf::from(&item.save_path);
                    if let Some(parent) = cur_dest.parent() {
                        let new_save_path = parent.join(&extracted_name);
                        item.filename = extracted_name;
                        item.save_path = new_save_path.to_string_lossy().to_string();
                    }
                }
            }
        }

        if total_bytes.is_none() {
            if let Some(cl) = response.headers().get(CONTENT_LENGTH) {
                if let Ok(cl_str) = cl.to_str() {
                    total_bytes = cl_str.parse::<u64>().ok();
                }
            }
        }

        let mut file = match OpenOptions::new()
            .create(true)
            .append(true)
            .open(&part_path)
        {
            Ok(f) => f,
            Err(e) => {
                let _ = app_handle.emit(
                    "download-progress",
                    DownloadProgressPayload {
                        id: item.id,
                        downloaded_bytes: existing_bytes,
                        total_bytes,
                        speed_bps: 0,
                        eta_seconds: None,
                        status: DownloadStatus::Error,
                        error_msg: Some(e.to_string()),
                    },
                );
                return;
            }
        };

        let mut stream = response.bytes_stream();
        let mut last_time = Instant::now();
        let mut bytes_since_last = 0u64;

        while let Some(chunk_result) = stream.next().await {
            if cancel_token.load(Ordering::SeqCst) {
                let _ = app_handle.emit(
                    "download-progress",
                    DownloadProgressPayload {
                        id: item.id,
                        downloaded_bytes: existing_bytes,
                        total_bytes,
                        speed_bps: 0,
                        eta_seconds: None,
                        status: DownloadStatus::Paused,
                        error_msg: None,
                    },
                );
                return;
            }

            let chunk = match chunk_result {
                Ok(c) => c,
                Err(e) => {
                    let _ = app_handle.emit(
                        "download-progress",
                        DownloadProgressPayload {
                            id: item.id,
                            downloaded_bytes: existing_bytes,
                            total_bytes,
                            speed_bps: 0,
                            eta_seconds: None,
                            status: DownloadStatus::Error,
                            error_msg: Some(e.to_string()),
                        },
                    );
                    return;
                }
            };

            if file.write_all(&chunk).is_err() {
                break;
            }

            existing_bytes += chunk.len() as u64;
            bytes_since_last += chunk.len() as u64;

            let now = Instant::now();
            let elapsed = now.duration_since(last_time).as_secs_f64();
            if elapsed >= 0.5 {
                let speed_bps = (bytes_since_last as f64 / elapsed) as u64;
                let eta = if let Some(tot) = total_bytes {
                    if speed_bps > 0 {
                        Some(tot.saturating_sub(existing_bytes) / speed_bps)
                    } else {
                        None
                    }
                } else {
                    None
                };

                let _ = app_handle.emit(
                    "download-progress",
                    DownloadProgressPayload {
                        id: item.id.clone(),
                        downloaded_bytes: existing_bytes,
                        total_bytes,
                        speed_bps,
                        eta_seconds: eta,
                        status: DownloadStatus::Downloading,
                        error_msg: None,
                    },
                );

                bytes_since_last = 0;
                last_time = now;
            }
        }

        // Finished stream
        let _ = fs::rename(&part_path, &item.save_path);
        let _ = app_handle.emit(
            "download-progress",
            DownloadProgressPayload {
                id: item.id,
                downloaded_bytes: existing_bytes,
                total_bytes: Some(existing_bytes),
                speed_bps: 0,
                eta_seconds: Some(0),
                status: DownloadStatus::Completed,
                error_msg: None,
            },
        );
    }

    fn normalize_video_url(url: &str) -> String {
        if let Some(pos) = url.find("/shorts/") {
            let prefix = &url[..pos];
            let rest = &url[pos + "/shorts/".len()..];
            if let Some(qpos) = rest.find('?') {
                let id = &rest[..qpos];
                let query = &rest[qpos + 1..];
                format!("{}/watch?v={}&{}", prefix, id, query)
            } else {
                format!("{}/watch?v={}", prefix, rest)
            }
        } else {
            url.to_string()
        }
    }

    async fn download_video_stream(
        app_handle: AppHandle,
        item: DownloadItem,
        cancel_token: Arc<AtomicBool>,
    ) {
        let quality = item.quality.as_deref().unwrap_or("best");
        let target_url = Self::normalize_video_url(&item.url);

        let mut args = vec![
            "--newline".to_string(),
            "--no-playlist".to_string(),
            "--user-agent".to_string(),
            BROWSER_USER_AGENT.to_string(),
            "--js-runtimes".to_string(),
            "node".to_string(),
            "--remote-components".to_string(),
            "ejs:github".to_string(),
            "--progress-template".to_string(),
            "%(progress._percent_str)s|%(progress._speed_str)s|%(progress._eta_str)s|%(progress.downloaded_bytes)s|%(progress.total_bytes)s".to_string(),
        ];

        if let Some(ref r) = item.referer {
            if !r.trim().is_empty() {
                args.push("--add-header".to_string());
                args.push(format!("Referer: {}", r));
            }
        }

        match quality {
            "audio" => {
                args.push("-x".to_string());
                args.push("--audio-format".to_string());
                args.push("mp3".to_string());
            }
            "1080" => {
                args.push("-f".to_string());
                args.push("bv*[height<=1080]+ba/b[height<=1080]/best".to_string());
                args.push("--merge-output-format".to_string());
                args.push("mp4".to_string());
            }
            "720" => {
                args.push("-f".to_string());
                args.push("bv*[height<=720]+ba/b[height<=720]/best".to_string());
                args.push("--merge-output-format".to_string());
                args.push("mp4".to_string());
            }
            _ => {
                args.push("-f".to_string());
                args.push("bv*+ba/b".to_string());
                args.push("--merge-output-format".to_string());
                args.push("mp4".to_string());
            }
        }

        args.push("-o".to_string());
        args.push(item.save_path.clone());
        args.push(target_url);

        let mut cmd = TokioCommand::new("yt-dlp");
        cmd.args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        #[cfg(target_os = "windows")]
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                let _ = app_handle.emit(
                    "download-progress",
                    DownloadProgressPayload {
                        id: item.id,
                        downloaded_bytes: 0,
                        total_bytes: None,
                        speed_bps: 0,
                        eta_seconds: None,
                        status: DownloadStatus::Error,
                        error_msg: Some(format!("yt-dlp launch error: {}", e)),
                    },
                );
                return;
            }
        };

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let stderr_lines = Arc::new(tokio::sync::Mutex::new(Vec::<String>::new()));
        let stderr_lines_clone = Arc::clone(&stderr_lines);

        if let Some(err_pipe) = stderr {
            tokio::spawn(async move {
                let mut reader = tokio::io::BufReader::new(err_pipe).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    let mut lines = stderr_lines_clone.lock().await;
                    if lines.len() > 30 {
                        lines.remove(0);
                    }
                    lines.push(line);
                }
            });
        }

        if let Some(out) = stdout {
            let mut reader = tokio::io::BufReader::new(out).lines();
            let app_h = app_handle.clone();
            let it_id = item.id.clone();
            let c_tok = Arc::clone(&cancel_token);

            while let Ok(Some(line)) = reader.next_line().await {
                if c_tok.load(Ordering::SeqCst) {
                    let _ = child.kill().await;
                    let _ = app_h.emit(
                        "download-progress",
                        DownloadProgressPayload {
                            id: it_id,
                            downloaded_bytes: 0,
                            total_bytes: None,
                            speed_bps: 0,
                            eta_seconds: None,
                            status: DownloadStatus::Paused,
                            error_msg: None,
                        },
                    );
                    return;
                }

                let parts: Vec<&str> = line.split('|').collect();
                if parts.len() >= 5 {
                    let downloaded_bytes = parts[3].trim().parse::<u64>().unwrap_or(0);
                    let total_bytes = parts[4].trim().parse::<u64>().ok();

                    let speed_str = parts[1].trim();
                    let speed_bps = Self::parse_speed_str(speed_str);

                    let eta_str = parts[2].trim();
                    let eta_seconds = Self::parse_eta_str(eta_str);

                    let _ = app_h.emit(
                        "download-progress",
                        DownloadProgressPayload {
                            id: it_id.clone(),
                            downloaded_bytes,
                            total_bytes,
                            speed_bps,
                            eta_seconds,
                            status: DownloadStatus::Downloading,
                            error_msg: None,
                        },
                    );
                } else if line.contains("[Merger]") || line.contains("Merging formats into") {
                    let _ = app_h.emit(
                        "download-progress",
                        DownloadProgressPayload {
                            id: it_id.clone(),
                            downloaded_bytes: 0,
                            total_bytes: None,
                            speed_bps: 0,
                            eta_seconds: None,
                            status: DownloadStatus::Merging,
                            error_msg: None,
                        },
                    );
                }
            }
        }

        let status = child.wait().await;
        if let Ok(st) = status {
            if st.success() {
                let file_len = fs::metadata(&item.save_path).map(|m| m.len()).unwrap_or(0);
                let _ = app_handle.emit(
                    "download-progress",
                    DownloadProgressPayload {
                        id: item.id,
                        downloaded_bytes: file_len,
                        total_bytes: Some(file_len),
                        speed_bps: 0,
                        eta_seconds: Some(0),
                        status: DownloadStatus::Completed,
                        error_msg: None,
                    },
                );
            } else {
                let errs = stderr_lines.lock().await;
                let last_err = errs
                    .iter()
                    .rev()
                    .find(|l| l.contains("ERROR:") || l.contains("Error") || !l.trim().is_empty())
                    .cloned()
                    .unwrap_or_else(|| "Video download or merge failed".to_string());

                let _ = app_handle.emit(
                    "download-progress",
                    DownloadProgressPayload {
                        id: item.id,
                        downloaded_bytes: 0,
                        total_bytes: None,
                        speed_bps: 0,
                        eta_seconds: None,
                        status: DownloadStatus::Error,
                        error_msg: Some(last_err),
                    },
                );
            }
        }
    }

    fn parse_speed_str(s: &str) -> u64 {
        let s = s.trim();
        if s.is_empty() || s == "N/A" {
            return 0;
        }

        let num_part: String = s.chars().take_while(|c| c.is_digit(10) || *c == '.').collect();
        let val: f64 = num_part.parse().unwrap_or(0.0);

        if s.contains("GiB") || s.contains("GB") {
            (val * 1024.0 * 1024.0 * 1024.0) as u64
        } else if s.contains("MiB") || s.contains("MB") {
            (val * 1024.0 * 1024.0) as u64
        } else if s.contains("KiB") || s.contains("KB") {
            (val * 1024.0) as u64
        } else {
            val as u64
        }
    }

    fn parse_eta_str(s: &str) -> Option<u64> {
        let s = s.trim();
        if s.is_empty() || s == "N/A" {
            return None;
        }

        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() == 2 {
            let mins: u64 = parts[0].parse().ok()?;
            let secs: u64 = parts[1].parse().ok()?;
            Some(mins * 60 + secs)
        } else if parts.len() == 3 {
            let hrs: u64 = parts[0].parse().ok()?;
            let mins: u64 = parts[1].parse().ok()?;
            let secs: u64 = parts[2].parse().ok()?;
            Some(hrs * 3600 + mins * 60 + secs)
        } else {
            None
        }
    }
}
