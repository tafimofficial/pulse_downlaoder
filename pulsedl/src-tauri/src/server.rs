use std::thread;
use tauri::{AppHandle, Emitter, Manager};
use tiny_http::{Header, Response, Server};

#[derive(serde::Deserialize, Debug)]
pub struct ExtensionDownloadRequest {
    pub url: String,
    pub filename: Option<String>,
    pub title: Option<String>,
    pub quality: Option<String>,
    pub is_video: Option<bool>,
    pub referer: Option<String>,
    pub page_url: Option<String>,
    pub cookies: Option<String>,
    pub file_size: Option<u64>,
}

pub fn start_extension_server(app_handle: AppHandle) {
    thread::spawn(move || {
        let server = match Server::http("127.0.0.1:45732") {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[PulseDL Server] Could not bind to 127.0.0.1:45732: {}", e);
                return;
            }
        };

        eprintln!("[PulseDL Server] Listening on 127.0.0.1:45732");

        loop {
            let mut request = match server.recv() {
                Ok(req) => req,
                Err(e) => {
                    eprintln!("[PulseDL Server] recv error (ignoring and staying alive): {}", e);
                    continue;
                }
            };

            let cors_origin = Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap();
            let cors_methods = Header::from_bytes(&b"Access-Control-Allow-Methods"[..], &b"GET, POST, OPTIONS"[..]).unwrap();
            let cors_headers = Header::from_bytes(&b"Access-Control-Allow-Headers"[..], &b"Content-Type, Authorization"[..]).unwrap();
            let content_type_json = Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap();

            let url = request.url().to_string();
            let method = request.method().as_str().to_string();

            if method == "OPTIONS" {
                let mut resp = Response::from_string("");
                resp.add_header(cors_origin);
                resp.add_header(cors_methods);
                resp.add_header(cors_headers);
                let _ = request.respond(resp);
                continue;
            }

            if (url == "/api/ping" || url.starts_with("/api/ping")) && method == "GET" {
                let mut resp = Response::from_string("{\"status\":\"ok\",\"version\":\"1.0.2\"}");
                resp.add_header(cors_origin);
                resp.add_header(content_type_json);
                let _ = request.respond(resp);
                continue;
            }

            if (url == "/api/show" || url.starts_with("/api/show")) && method == "GET" {
                let app_h = app_handle.clone();
                let _ = app_handle.run_on_main_thread(move || {
                    if let Some(win) = app_h.get_webview_window("main") {
                        let _ = win.unminimize();
                        let _ = win.show();
                        let _ = win.set_focus();
                    } else {
                        let _ = tauri::WebviewWindowBuilder::new(&app_h, "main", tauri::WebviewUrl::App("index.html".into()))
                            .title("PulseDL - Ultra Fast Turbo Downloader")
                            .inner_size(1080.0, 700.0)
                            .min_inner_size(840.0, 540.0)
                            .center()
                            .decorations(true)
                            .visible(true)
                            .build();
                    }
                });
                let mut resp = Response::from_string("{\"status\":\"ok\",\"shown\":true}");
                resp.add_header(cors_origin);
                resp.add_header(content_type_json);
                let _ = request.respond(resp);
                continue;
            }

            if (url == "/api/download" || url.starts_with("/api/download")) && method == "POST" {
                let mut body_str = String::new();
                if let Some(len) = request.body_length() {
                    if len > 0 {
                        let mut buf = vec![0u8; len];
                        if request.as_reader().read_exact(&mut buf).is_ok() {
                            body_str = String::from_utf8_lossy(&buf).to_string();
                        }
                    }
                } else {
                    let _ = std::io::Read::read_to_string(&mut request.as_reader(), &mut body_str);
                }

                if let Ok(req) = serde_json::from_str::<ExtensionDownloadRequest>(&body_str) {
                    let chosen_filename = req.filename.or(req.title);
                    let chosen_referer = req.referer.or(req.page_url);

                    let _ = app_handle.emit("extension-add-download", serde_json::json!({
                        "url": req.url,
                        "filename": chosen_filename,
                        "quality": req.quality.unwrap_or_else(|| "best".to_string()),
                        "is_video": req.is_video.unwrap_or(false),
                        "referer": chosen_referer,
                        "cookies": req.cookies,
                        "file_size": req.file_size
                    }));

                    let mut resp = Response::from_string("{\"success\":true,\"message\":\"Download added to PulseDL\"}");
                    resp.add_header(cors_origin);
                    resp.add_header(content_type_json);
                    let _ = request.respond(resp);
                    continue;
                }

                let mut resp = Response::from_string("{\"error\":\"Invalid request body\"}");
                resp.add_header(cors_origin);
                resp.add_header(content_type_json);
                let _ = request.respond(resp);
                continue;
            }

            let mut resp = Response::from_string("{\"error\":\"Not found\"}");
            resp.add_header(cors_origin);
            resp.add_header(content_type_json);
            let _ = request.respond(resp);
        }
    });
}
