use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

pub fn open_download_popup(app: &AppHandle, id: &str, filename: &str) {
    let app_h = app.clone();
    let id_str = id.to_string();
    let file_str = filename.to_string();

    let _ = app.run_on_main_thread(move || {
        let window_label = format!("popup_{}", id_str);

        if let Some(win) = app_h.get_webview_window(&window_label) {
            let _ = win.unminimize();
            let _ = win.show();
            let _ = win.set_focus();
            let _ = win.set_always_on_top(true);
            return;
        }

        let title_str = format!("Downloading: {}", file_str);

        if let Ok(win) = WebviewWindowBuilder::new(&app_h, &window_label, WebviewUrl::App("index.html".into()))
            .title(title_str)
            .inner_size(480.0, 250.0)
            .resizable(false)
            .maximizable(false)
            .center()
            .always_on_top(true)
            .decorations(true)
            .build()
        {
            let _ = win.unminimize();
            let _ = win.show();
            let _ = win.set_focus();
        }
    });
}
