# ⚡ PulseDL — Ultra Fast & Lightweight Turbo Download Manager

**PulseDL** is an ultra-fast, lightweight download manager built with a **Rust core engine** and **Tauri 2 UI**, styled in an electric **"Neon Lime + Pitch Black"** theme.

---

## 🚀 Key Highlights & Features

### 1. ⚡ 99% Bandwidth Saturation (Turbo Multi-Connection Engine)
* **32 Parallel TCP Streams:** Slices large files into multiple chunks and downloads them simultaneously via HTTP Range headers.
* **Aggressive Socket Buffers:** Maximizes TCP window sizes and network pipe utilization so your PC pulls up to 99% of available bandwidth.
* **Speed Limiter:** Switch between Unlimited (Turbo 100% blast) or custom speed caps (2 MB/s, 5 MB/s, 10 MB/s, 25 MB/s, 50 MB/s).

### 2. 💾 Dynamic Storage Allocation (No Pre-allocation)
* **Zero Dummy Disk Space:** Files **never** allocate full storage in advance. If a 10 GB file has only downloaded 200 MB, it occupies **only 200 MB** on your drive.
* Uses dynamic streaming chunk files (`.part0`, `.part1`...) that grow strictly as bytes arrive from the network and assemble instantly once complete.

### 3. 🎬 Universal Video Downloader (YouTube, Facebook, TikTok, & More)
* **Automatic Video & Audio Separation & Merging:** Extracts high-bitrate video streams and high-fidelity audio streams separately from YouTube, Facebook, TikTok, etc., and merges them seamlessly with **FFmpeg** into MP4/MKV.
* **Format Selector:** Choose between Best Quality (4K/1080p), 1080p FHD, 720p HD, or Audio-Only (MP3).

### 4. 🪟 Dedicated Floating Mini Popup per Download
* Each active download has its own dedicated, sleek floating popup window displaying:
  * File name & category icon
  * Animated neon-lime progress bar (% complete)
  * Real-time download speed (e.g., `32.5 MB/s`) and ETA
  * Downloaded vs Total size (e.g., `450 MB / 1.2 GB`)
  * Control buttons: **Pause**, **Resume**, **Open File**, **Open Folder**, and **Cancel**.
  * Closing the popup keeps the download running safely in the background.

### 5. 🖥️ Main Window & Advanced Batch Management
* **Comprehensive History List:** All active, paused, and completed downloads saved persistently in local storage across PC restarts.
* **Drag-and-Select & Multi-Selection:** Select rows via drag-selection, Shift+Click, Ctrl+Click, or row checkboxes.
* **Batch Actions:** Batch Pause, Batch Resume, Batch Delete (from list or permanently from disk).
* **Search & Filter:** Instant search by name or URL; filter tabs for All, Downloading, Paused, and Completed.
* **Native Folder Picker:** Easily change your default or per-download destination folder.

### 6. 🛡️ System Tray & Silent Windows Boot
* **Minimize-to-Tray:** Closing the main window (`X`) minimizes the application to the Windows System Tray without interrupting downloads.
* **Silent Autostart:** When enabled in Settings, PulseDL starts automatically on Windows boot silently inside the System Tray without showing any window.
* **Tray Context Menu:** Open window, Pause All, Resume All, or completely Exit.

### 7. 🧩 Browser Extension (Chrome, Edge, Brave, Opera)
* **IDM-Style Floating Video Grabber:**
  * When any video plays on YouTube, Facebook, TikTok, etc., a compact button floats in:  
    `[ ▶ Download this video  | ▼ | ✕ ]`
  * Automatically fades away after 2.5–3 seconds if untouched.
  * Click `✕` to dismiss instantly.
  * Click to select format (Best Quality, 1080p, 720p, MP3 Audio).
  * Can be toggled on/off in extension options.
* **Browser Download Interception:**
  * Automatically intercepts large file downloads (.zip, .exe, .iso, .mp4, .mkv, .tar.gz, etc.) and routes them to PulseDL.
  * Can be turned on or off via extension popup.

---

## 📁 Executable Files Location

Both the native setup installer and standalone executable are compiled and ready in `d:\faster download`:

| File | Type | Size | Description |
|---|---|---|---|
| **`PulseDL_Setup.exe`** | Native Installer | ~2.36 MB | Official Windows Setup Installer with Desktop & Start Menu shortcuts |
| **`PulseDL.exe`** | Native Binary | ~6.80 MB | Standalone executable (direct run) |

---

## 🌐 Browser Extension Installation (10 seconds)

1. Open **Google Chrome** or **Microsoft Edge**.
2. Navigate to: `chrome://extensions` (or `edge://extensions`).
3. Enable **Developer mode** (toggle in the top-right corner).
4. Click **Load unpacked** (in top-left corner).
5. Select the folder:  
   `D:\faster download\extension`
6. Done! The PulseDL Turbo Grabber icon will appear in your browser toolbar, and the floating video download button will automatically appear on video players.
