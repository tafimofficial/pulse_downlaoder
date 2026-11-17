import React, { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import {
  Zap,
  Play,
  Pause,
  Trash2,
  Plus,
  Settings,
  Folder,
  FileVideo,
  FileAudio,
  FileText,
  FileArchive,
  File as FileIcon,
  CheckCircle2,
  Search,
  Maximize2,
  FolderOpen,
  X,
  Gauge,
  CheckSquare,
  Square,
  Sparkles,
} from "lucide-react";
import "./App.css";

interface DownloadItem {
  id: string;
  url: string;
  filename: string;
  save_path: string;
  total_bytes: number | null;
  downloaded_bytes: number;
  status: "queued" | "downloading" | "paused" | "merging" | "completed" | "error";
  speed_bps: number;
  eta_seconds: number | null;
  is_video: boolean;
  quality?: string;
  connections: number;
  error_msg?: string;
  created_at: string;
}

interface AppSettings {
  default_download_dir: string;
  speed_limit_bps: number;
  autostart: boolean;
  minimize_to_tray: boolean;
  show_popup_on_download: boolean;
  max_active_downloads: number;
  default_connections: number;
}

function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  const k = 1024;
  const sizes = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + " " + sizes[i];
}

function formatSpeed(bps: number): string {
  if (bps === 0) return "0 KB/s";
  return `${formatBytes(bps)}/s`;
}

function formatEta(seconds: number | null): string {
  if (!seconds || seconds <= 0) return "--:--";
  const m = Math.floor(seconds / 60);
  const s = seconds % 60;
  if (m > 60) {
    const h = Math.floor(m / 60);
    const rm = m % 60;
    return `${h}h ${rm}m`;
  }
  return `${m}m ${s < 10 ? "0" + s : s}s`;
}

function getFileIcon(filename: string, is_video: boolean) {
  if (is_video) return <FileVideo size={18} />;
  const ext = filename.split(".").pop()?.toLowerCase() || "";
  if (["mp4", "mkv", "avi", "mov", "webm", "flv"].includes(ext)) {
    return <FileVideo size={18} />;
  }
  if (["mp3", "flac", "wav", "aac", "ogg", "m4a"].includes(ext)) {
    return <FileAudio size={18} />;
  }
  if (["zip", "rar", "7z", "tar", "gz", "iso"].includes(ext)) {
    return <FileArchive size={18} />;
  }
  if (["pdf", "doc", "docx", "txt", "xlsx"].includes(ext)) {
    return <FileText size={18} />;
  }
  return <FileIcon size={18} />;
}

export default function App() {
  const [winLabel, setWinLabel] = useState<string>("main");
  const urlParams = new URLSearchParams(window.location.search);

  useEffect(() => {
    try {
      const current = getCurrentWebviewWindow();
      if (current && current.label) {
        setWinLabel(current.label);
      }
    } catch {
      // preview outside Tauri
    }
  }, []);

  const isPopup = winLabel.startsWith("popup_") || urlParams.get("popup") === "true";
  const popupId = winLabel.startsWith("popup_") ? winLabel.replace("popup_", "") : urlParams.get("id");

  if (isPopup && popupId) {
    return <MiniPopupWindow id={popupId} />;
  }

  return <MainWindow />;
}

/* MINI POPUP WINDOW */
function MiniPopupWindow({ id }: { id: string }) {
  const [item, setItem] = useState<DownloadItem | null>(null);

  const loadItem = async () => {
    try {
      const list = await invoke<DownloadItem[]>("get_downloads");
      const cleanId = id.replace(/-/g, "");
      const found = list.find((i) => i.id === id || i.id.replace(/-/g, "") === cleanId);
      if (found) setItem(found);
    } catch (err) {
      console.error(err);
    }
  };

  useEffect(() => {
    loadItem();
    const interval = setInterval(() => {
      setItem((prev) => {
        if (!prev) loadItem();
        return prev;
      });
    }, 400);

    const cleanId = id.replace(/-/g, "");
    const unlistenProgress = listen<any>("download-progress", (event) => {
      const payload = event.payload;
      if (payload.id === id || (payload.id && payload.id.replace(/-/g, "") === cleanId)) {
        setItem((prev) => {
          if (!prev) return null;
          return {
            ...prev,
            downloaded_bytes: payload.downloaded_bytes,
            total_bytes: payload.total_bytes ?? prev.total_bytes,
            speed_bps: payload.speed_bps,
            eta_seconds: payload.eta_seconds,
            status: payload.status,
            error_msg: payload.error_msg,
          };
        });
      }
    });

    return () => {
      clearInterval(interval);
      unlistenProgress.then((fn) => fn());
    };
  }, [id]);

  if (!item) {
    return (
      <div className="popup-body" style={{ alignItems: "center", justifyContent: "center" }}>
        <div style={{ color: "var(--lime)" }}>Loading download details...</div>
      </div>
    );
  }

  const isCompleted = item.status === "completed";
  const displayDownloaded = item.total_bytes
    ? Math.min(item.downloaded_bytes, item.total_bytes)
    : item.downloaded_bytes;

  const percent =
    item.total_bytes && item.total_bytes > 0
      ? Math.min(100, Math.max(0, (displayDownloaded / item.total_bytes) * 100))
      : isCompleted
      ? 100
      : 0;

  const togglePause = async () => {
    if (item.status === "downloading") {
      await invoke("pause_download", { id: item.id });
    } else {
      await invoke("resume_download", { id: item.id });
    }
    loadItem();
  };

  return (
    <div className="popup-body">
      <div className="popup-top">
        <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
          <div style={{ color: "var(--lime)" }}>{getFileIcon(item.filename, item.is_video)}</div>
          <div className="popup-title" title={item.filename}>
            {item.filename}
          </div>
        </div>
        <span className={`status-pill status-${item.status}`}>{item.status}</span>
      </div>

      <div className="popup-progress-box">
        <div className="progress-track" style={{ height: "10px" }}>
          <div className="progress-fill" style={{ width: `${percent}%` }}></div>
        </div>
        <div className="popup-metrics">
          <div>
            <b>{percent.toFixed(1)}%</b> ({formatBytes(displayDownloaded)} /{" "}
            {item.total_bytes ? formatBytes(item.total_bytes) : "Unknown"})
          </div>
          <div>
            Speed: <b>{formatSpeed(item.speed_bps)}</b> • ETA: <b>{formatEta(item.eta_seconds)}</b>
          </div>
        </div>
      </div>

      <div className="popup-actions">
        <div style={{ display: "flex", gap: "8px" }}>
          {isCompleted ? (
            <button
              className="btn btn-primary"
              style={{ padding: "6px 14px", fontSize: "12px" }}
              onClick={() => invoke("open_file", { path: item.save_path })}
            >
              <CheckCircle2 size={14} /> Open File
            </button>
          ) : (
            <button
              className="btn btn-primary"
              style={{ padding: "6px 14px", fontSize: "12px" }}
              onClick={togglePause}
            >
              {item.status === "downloading" ? (
                <>
                  <Pause size={14} /> Pause
                </>
              ) : (
                <>
                  <Play size={14} /> Resume
                </>
              )}
            </button>
          )}
          <button
            className="btn btn-secondary"
            style={{ padding: "6px 12px", fontSize: "12px" }}
            onClick={() => invoke("open_folder", { path: item.save_path })}
          >
            <FolderOpen size={14} /> Folder
          </button>
        </div>

        {isCompleted ? (
          <button
            className="btn btn-secondary"
            style={{ padding: "6px 14px", fontSize: "12px" }}
            onClick={async () => {
              try {
                await invoke("close_popup_window", { id: item.id });
              } catch (e) {
                console.error(e);
              }
              try {
                const win = getCurrentWebviewWindow();
                await win.destroy();
              } catch {}
            }}
          >
            <X size={14} /> Close
          </button>
        ) : (
          <button
            className="btn btn-danger"
            style={{ padding: "6px 12px", fontSize: "12px" }}
            onClick={async () => {
              try {
                await invoke("cancel_download", { id: item.id, deleteFile: true });
              } catch (e) {
                console.error(e);
              }
              try {
                await invoke("close_popup_window", { id: item.id });
              } catch (e) {
                console.error(e);
              }
              try {
                const win = getCurrentWebviewWindow();
                await win.destroy();
              } catch {}
            }}
          >
            <Trash2 size={14} /> Cancel
          </button>
        )}
      </div>
    </div>
  );
}

/* MAIN APPLICATION WINDOW */
function MainWindow() {
  const [downloads, setDownloads] = useState<DownloadItem[]>([]);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [activeFilter, setActiveFilter] = useState<string>("all");
  const [searchQuery, setSearchQuery] = useState<string>("");
  const [isAddModalOpen, setIsAddModalOpen] = useState(false);
  const [isSettingsModalOpen, setIsSettingsModalOpen] = useState(false);
  const [settings, setSettings] = useState<AppSettings | null>(null);

  // Add download modal form
  const [newUrl, setNewUrl] = useState("");
  const [newFilename, setNewFilename] = useState("");
  const [newSaveDir, setNewSaveDir] = useState("");
  const [newQuality, setNewQuality] = useState("best");
  const [newConnections, setNewConnections] = useState(16);

  const fetchDownloads = async () => {
    try {
      const list = await invoke<DownloadItem[]>("get_downloads");
      setDownloads(list);
    } catch (err) {
      console.error(err);
    }
  };

  const fetchSettings = async () => {
    try {
      const s = await invoke<AppSettings>("get_settings");
      setSettings(s);
      setNewSaveDir(s.default_download_dir);
    } catch (err) {
      console.error(err);
    }
  };

  useEffect(() => {
    fetchDownloads();
    fetchSettings();

    const unlistenProgress = listen<any>("download-progress", (event) => {
      const payload = event.payload;
      setDownloads((prev) => {
        const found = prev.some((item) => item.id === payload.id);
        if (!found) {
          fetchDownloads();
          return prev;
        }
        return prev.map((item) => {
          if (item.id === payload.id) {
            return {
              ...item,
              downloaded_bytes: payload.downloaded_bytes,
              total_bytes: payload.total_bytes ?? item.total_bytes,
              speed_bps: payload.speed_bps,
              eta_seconds: payload.eta_seconds,
              status: payload.status,
              error_msg: payload.error_msg,
            };
          }
          return item;
        });
      });
    });

    const unlistenAdded = listen<DownloadItem>("download-added", (event) => {
      setDownloads((prev) => {
        if (prev.some((i) => i.id === event.payload.id)) return prev;
        return [event.payload, ...prev];
      });
    });

    const unlistenCompleted = listen<string>("download-completed", () => {
      fetchDownloads();
    });

    const unlistenTrayPause = listen("tray-pause-all", () => {
      invoke("pause_all").then(() => fetchDownloads());
    });

    const unlistenTrayResume = listen("tray-resume-all", () => {
      invoke("resume_all").then(() => fetchDownloads());
    });

    const pollInterval = setInterval(() => {
      fetchDownloads();
    }, 1500);

    return () => {
      clearInterval(pollInterval);
      unlistenProgress.then((fn) => fn());
      unlistenAdded.then((fn) => fn());
      unlistenCompleted.then((fn) => fn());
      unlistenTrayPause.then((fn) => fn());
      unlistenTrayResume.then((fn) => fn());
    };
  }, []);

  // Aggregated live speed
  const totalSpeedBps = downloads
    .filter((d) => d.status === "downloading")
    .reduce((acc, curr) => acc + (curr.speed_bps || 0), 0);

  const activeCount = downloads.filter((d) => d.status === "downloading").length;
  const completedCount = downloads.filter((d) => d.status === "completed").length;

  // Filter downloads
  const filteredDownloads = downloads.filter((item) => {
    if (activeFilter === "downloading" && item.status !== "downloading") return false;
    if (activeFilter === "completed" && item.status !== "completed") return false;
    if (activeFilter === "paused" && item.status !== "paused") return false;

    if (searchQuery.trim()) {
      const q = searchQuery.toLowerCase();
      return (
        item.filename.toLowerCase().includes(q) ||
        item.url.toLowerCase().includes(q) ||
        item.status.toLowerCase().includes(q)
      );
    }
    return true;
  });

  // Multi-selection handlers
  const toggleSelectRow = (id: string, e: React.MouseEvent) => {
    e.stopPropagation();
    const next = new Set(selectedIds);
    if (next.has(id)) {
      next.delete(id);
    } else {
      next.add(id);
    }
    setSelectedIds(next);
  };

  const selectAll = () => {
    if (selectedIds.size === filteredDownloads.length) {
      setSelectedIds(new Set());
    } else {
      setSelectedIds(new Set(filteredDownloads.map((i) => i.id)));
    }
  };

  const handleBatchPause = async () => {
    await invoke("batch_pause", { ids: Array.from(selectedIds) });
    fetchDownloads();
  };

  const handleBatchResume = async () => {
    await invoke("batch_resume", { ids: Array.from(selectedIds) });
    fetchDownloads();
  };

  const handleBatchDelete = async (deleteFiles: boolean) => {
    await invoke("batch_delete", {
      ids: Array.from(selectedIds),
      deleteFiles,
    });
    setSelectedIds(new Set());
    fetchDownloads();
  };

  const handleOpenAddModal = async () => {
    try {
      const text = await navigator.clipboard.readText();
      if (text && (text.startsWith("http://") || text.startsWith("https://"))) {
        setNewUrl(text);
      }
    } catch {}
    setIsAddModalOpen(true);
  };

  const handleStartDownload = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!newUrl.trim()) return;

    await invoke("add_download", {
      url: newUrl.trim(),
      filename: newFilename.trim() ? newFilename.trim() : null,
      saveDir: newSaveDir.trim() ? newSaveDir.trim() : null,
      isVideo:
        newUrl.includes("youtube.com") ||
        newUrl.includes("youtu.be") ||
        newUrl.includes("facebook.com") ||
        newUrl.includes("tiktok.com"),
      quality: newQuality,
      connections: newConnections,
      cookies: null,
      referer: null,
    });

    setNewUrl("");
    setNewFilename("");
    setIsAddModalOpen(false);
    fetchDownloads();
  };

  const handleBrowseFolder = async () => {
    const selected = await invoke<string | null>("pick_folder");
    if (selected) {
      setNewSaveDir(selected);
    }
  };

  const handleSaveSettings = async () => {
    if (!settings) return;
    await invoke("update_settings", { settings });
    setIsSettingsModalOpen(false);
  };

  return (
    <div className="app-container">
      {/* HEADER */}
      <header className="app-header">
        <div className="logo-section">
          <div className="logo-icon-box">
            <Zap size={22} />
          </div>
          <div>
            <div className="logo-text">
              <span>PulseDL</span>
              <span className="logo-tag">TURBO</span>
            </div>
          </div>
        </div>

        <div className="header-stats">
          <div className="stat-pill turbo">
            <span className="pulsing-dot"></span>
            <span>
              <b>99% Bandwidth Saturation Active</b>
            </span>
          </div>
          <div className="stat-pill">
            <Gauge size={15} color="var(--lime)" />
            <span>
              Total Speed: <b>{formatSpeed(totalSpeedBps)}</b>
            </span>
          </div>
          <div className="stat-pill">
            <span>
              Active: <b>{activeCount}</b> • Done: <b>{completedCount}</b>
            </span>
          </div>
        </div>
      </header>

      {/* TOOLBAR */}
      <div className="toolbar">
        <div className="toolbar-left">
          <button className="btn btn-primary" onClick={handleOpenAddModal}>
            <Plus size={16} />
            <span>Add URL</span>
          </button>
          <button
            className="btn btn-secondary"
            onClick={() => invoke("resume_all").then(() => fetchDownloads())}
          >
            <Play size={15} /> Resume All
          </button>
          <button
            className="btn btn-secondary"
            onClick={() => invoke("pause_all").then(() => fetchDownloads())}
          >
            <Pause size={15} /> Pause All
          </button>
          <button
            className="btn btn-secondary"
            onClick={() => invoke("clear_completed").then(() => fetchDownloads())}
          >
            <CheckCircle2 size={15} /> Clear Completed
          </button>
        </div>

        <div style={{ display: "flex", alignItems: "center", gap: "10px" }}>
          <button
            className="btn btn-secondary"
            onClick={() =>
              settings && invoke("open_folder", { path: settings.default_download_dir })
            }
            title="Open Downloads Folder"
          >
            <FolderOpen size={16} /> Folder
          </button>
          <button
            className="btn btn-secondary btn-icon"
            onClick={() => setIsSettingsModalOpen(true)}
            title="Settings"
          >
            <Settings size={18} />
          </button>
        </div>
      </div>

      {/* SELECTION ACTIONS BANNER */}
      {selectedIds.size > 0 && (
        <div className="selection-banner">
          <div style={{ display: "flex", alignItems: "center", gap: "12px", fontSize: "13px" }}>
            <span style={{ color: "var(--lime)", fontWeight: "700" }}>
              {selectedIds.size} {selectedIds.size === 1 ? "item" : "items"} selected
            </span>
          </div>
          <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
            <button
              className="btn btn-secondary"
              style={{ padding: "5px 12px", fontSize: "12px" }}
              onClick={handleBatchResume}
            >
              <Play size={13} /> Resume
            </button>
            <button
              className="btn btn-secondary"
              style={{ padding: "5px 12px", fontSize: "12px" }}
              onClick={handleBatchPause}
            >
              <Pause size={13} /> Pause
            </button>
            <button
              className="btn btn-danger"
              style={{ padding: "5px 12px", fontSize: "12px" }}
              onClick={() => handleBatchDelete(false)}
            >
              <Trash2 size={13} /> Delete from List
            </button>
            <button
              className="btn btn-danger"
              style={{ padding: "5px 12px", fontSize: "12px" }}
              onClick={() => handleBatchDelete(true)}
            >
              <Trash2 size={13} /> Delete File
            </button>
            <button
              className="btn btn-secondary"
              style={{ padding: "5px 10px", fontSize: "12px" }}
              onClick={() => setSelectedIds(new Set())}
            >
              <X size={14} />
            </button>
          </div>
        </div>
      )}

      {/* FILTER & SEARCH BAR */}
      <div className="filter-bar">
        <div className="tab-group">
          <button
            className={`tab-btn ${activeFilter === "all" ? "active" : ""}`}
            onClick={() => setActiveFilter("all")}
          >
            All ({downloads.length})
          </button>
          <button
            className={`tab-btn ${activeFilter === "downloading" ? "active" : ""}`}
            onClick={() => setActiveFilter("downloading")}
          >
            Downloading ({activeCount})
          </button>
          <button
            className={`tab-btn ${activeFilter === "paused" ? "active" : ""}`}
            onClick={() => setActiveFilter("paused")}
          >
            Paused ({downloads.filter((d) => d.status === "paused").length})
          </button>
          <button
            className={`tab-btn ${activeFilter === "completed" ? "active" : ""}`}
            onClick={() => setActiveFilter("completed")}
          >
            Completed ({completedCount})
          </button>
        </div>

        <div className="search-box">
          <Search size={14} color="var(--text-dim)" />
          <input
            type="text"
            placeholder="Search downloads..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
          />
        </div>
      </div>

      {/* DOWNLOADS TABLE */}
      <div className="table-container">
        {filteredDownloads.length === 0 ? (
          <div className="empty-state">
            <div className="empty-icon">
              <Zap size={30} />
            </div>
            <h3 style={{ marginBottom: "6px", color: "var(--text-main)" }}>No Downloads Found</h3>
            <p style={{ fontSize: "13px", maxWidth: "340px", marginBottom: "16px" }}>
              Click "+ Add URL" or grab videos directly from YouTube, Facebook, and TikTok using the
              PulseDL Browser Extension!
            </p>
            <button className="btn btn-primary" onClick={handleOpenAddModal}>
              <Plus size={16} /> Add Download
            </button>
          </div>
        ) : (
          <table className="download-table">
            <thead>
              <tr>
                <th style={{ width: "36px" }}>
                  <div
                    onClick={selectAll}
                    style={{ cursor: "pointer", display: "flex", alignItems: "center" }}
                  >
                    {selectedIds.size > 0 && selectedIds.size === filteredDownloads.length ? (
                      <CheckSquare size={16} color="var(--lime)" />
                    ) : (
                      <Square size={16} color="var(--text-dim)" />
                    )}
                  </div>
                </th>
                <th>File Name</th>
                <th>Status</th>
                <th>Size / Progress</th>
                <th>Speed</th>
                <th>ETA</th>
                <th style={{ textAlign: "right" }}>Actions</th>
              </tr>
            </thead>
            <tbody>
              {filteredDownloads.map((item) => {
                const isSelected = selectedIds.has(item.id);
                const percent =
                  item.total_bytes && item.total_bytes > 0
                    ? Math.min(100, (item.downloaded_bytes / item.total_bytes) * 100)
                    : 0;

                return (
                  <tr
                    key={item.id}
                    className={`download-row ${isSelected ? "selected" : ""}`}
                    onClick={(e) => toggleSelectRow(item.id, e)}
                  >
                    <td>
                      <div
                        onClick={(e) => toggleSelectRow(item.id, e)}
                        style={{ display: "flex", alignItems: "center" }}
                      >
                        {isSelected ? (
                          <CheckSquare size={16} color="var(--lime)" />
                        ) : (
                          <Square size={16} color="var(--text-dim)" />
                        )}
                      </div>
                    </td>

                    <td>
                      <div className="file-info">
                        <div className="file-icon-box">
                          {getFileIcon(item.filename, item.is_video)}
                        </div>
                        <div className="file-text">
                          <div className="file-name" title={item.filename}>
                            {item.filename}
                          </div>
                          <div className="file-meta">
                            {item.is_video ? "Video Stream • " : "Direct File • "}
                            {item.connections} threads
                          </div>
                        </div>
                      </div>
                    </td>

                    <td>
                      <span className={`status-pill status-${item.status}`}>
                        {item.status === "downloading" && <span className="pulsing-dot"></span>}
                        {item.status}
                      </span>
                    </td>

                    <td>
                      <div className="progress-container">
                        <div className="progress-track">
                          <div className="progress-fill" style={{ width: `${percent}%` }}></div>
                        </div>
                        <div className="progress-text">
                          <span>{percent.toFixed(1)}%</span>
                          <span>
                            {formatBytes(item.downloaded_bytes)} /{" "}
                            {item.total_bytes ? formatBytes(item.total_bytes) : "Unknown"}
                          </span>
                        </div>
                      </div>
                    </td>

                    <td style={{ color: item.speed_bps > 0 ? "var(--lime)" : "inherit" }}>
                      <b>{formatSpeed(item.speed_bps)}</b>
                    </td>

                    <td>{formatEta(item.eta_seconds)}</td>

                    <td>
                      <div className="row-actions" style={{ justifyContent: "flex-end" }}>
                        {item.status === "downloading" ? (
                          <button
                            className="action-btn"
                            title="Pause"
                            onClick={(e) => {
                              e.stopPropagation();
                              invoke("pause_download", { id: item.id }).then(() => fetchDownloads());
                            }}
                          >
                            <Pause size={14} />
                          </button>
                        ) : item.status !== "completed" ? (
                          <button
                            className="action-btn"
                            title="Resume"
                            onClick={(e) => {
                              e.stopPropagation();
                              invoke("resume_download", { id: item.id }).then(() => fetchDownloads());
                            }}
                          >
                            <Play size={14} />
                          </button>
                        ) : (
                          <button
                            className="action-btn"
                            title="Open File"
                            onClick={(e) => {
                              e.stopPropagation();
                              invoke("open_file", { path: item.save_path });
                            }}
                          >
                            <CheckCircle2 size={14} color="var(--lime)" />
                          </button>
                        )}

                        <button
                          className="action-btn"
                          title="Open Floating Mini Popup"
                          onClick={(e) => {
                            e.stopPropagation();
                            invoke("open_popup_window", { id: item.id });
                          }}
                        >
                          <Maximize2 size={13} />
                        </button>

                        <button
                          className="action-btn"
                          title="Open Folder"
                          onClick={(e) => {
                            e.stopPropagation();
                            invoke("open_folder", { path: item.save_path });
                          }}
                        >
                          <Folder size={14} />
                        </button>

                        <button
                          className="action-btn delete"
                          title="Delete"
                          onClick={(e) => {
                            e.stopPropagation();
                            invoke("cancel_download", { id: item.id, deleteFile: false }).then(() =>
                              fetchDownloads()
                            );
                          }}
                        >
                          <Trash2 size={14} />
                        </button>
                      </div>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        )}
      </div>

      {/* FOOTER */}
      <footer className="app-footer">
        <div>PulseDL v1.0 • Core: Rust + Tauri 2 • 99% Bandwidth Saturation Engine</div>
        <div style={{ display: "flex", gap: "14px" }}>
          <span>Browser Extension: Port 45732 (Active)</span>
          <span>Storage: Dynamic Growth (Zero Dummy Allocation)</span>
        </div>
      </footer>

      {/* ADD DOWNLOAD MODAL */}
      {isAddModalOpen && (
        <div className="modal-overlay" onClick={() => setIsAddModalOpen(false)}>
          <div className="modal-card" onClick={(e) => e.stopPropagation()}>
            <div className="modal-header">
              <div className="modal-title">
                <Sparkles size={18} color="var(--lime)" />
                <span>Add Turbo Download</span>
              </div>
              <button
                className="action-btn"
                onClick={() => setIsAddModalOpen(false)}
                style={{ padding: "4px" }}
              >
                <X size={16} />
              </button>
            </div>

            <form onSubmit={handleStartDownload}>
              <div className="form-group">
                <label className="form-label">Download URL (Video or Direct File)</label>
                <input
                  type="text"
                  className="form-input"
                  placeholder="https://www.youtube.com/watch?v=... or direct link"
                  value={newUrl}
                  onChange={(e) => setNewUrl(e.target.value)}
                  required
                  autoFocus
                />
              </div>

              <div className="form-group">
                <label className="form-label">Custom File Name (Optional)</label>
                <input
                  type="text"
                  className="form-input"
                  placeholder="Auto-detected from URL if left empty"
                  value={newFilename}
                  onChange={(e) => setNewFilename(e.target.value)}
                />
              </div>

              <div className="form-group">
                <label className="form-label">Save Directory</label>
                <div style={{ display: "flex", gap: "8px" }}>
                  <input
                    type="text"
                    className="form-input"
                    value={newSaveDir}
                    onChange={(e) => setNewSaveDir(e.target.value)}
                  />
                  <button
                    type="button"
                    className="btn btn-secondary"
                    onClick={handleBrowseFolder}
                    style={{ whiteSpace: "nowrap" }}
                  >
                    Browse...
                  </button>
                </div>
              </div>

              <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "14px" }}>
                <div className="form-group">
                  <label className="form-label">Video Quality / Format</label>
                  <select
                    className="form-input"
                    value={newQuality}
                    onChange={(e) => setNewQuality(e.target.value)}
                  >
                    <option value="best">⚡ Best Quality (Merged 4K/1080p)</option>
                    <option value="1080">🎬 1080p Full HD</option>
                    <option value="720">🎬 720p HD</option>
                    <option value="audio">🎵 Audio Only (MP3)</option>
                  </select>
                </div>

                <div className="form-group">
                  <label className="form-label">Parallel Connections</label>
                  <select
                    className="form-input"
                    value={newConnections}
                    onChange={(e) => setNewConnections(Number(e.target.value))}
                  >
                    <option value={32}>32 Threads (99% Extreme Pipe)</option>
                    <option value={16}>16 Threads (Turbo Fast)</option>
                    <option value={8}>8 Threads (Balanced)</option>
                    <option value={4}>4 Threads (Light)</option>
                  </select>
                </div>
              </div>

              <div className="modal-actions">
                <button
                  type="button"
                  className="btn btn-secondary"
                  onClick={() => setIsAddModalOpen(false)}
                >
                  Cancel
                </button>
                <button type="submit" className="btn btn-primary">
                  <Zap size={15} /> Start Download
                </button>
              </div>
            </form>
          </div>
        </div>
      )}

      {/* SETTINGS MODAL */}
      {isSettingsModalOpen && settings && (
        <div className="modal-overlay" onClick={() => setIsSettingsModalOpen(false)}>
          <div className="modal-card" onClick={(e) => e.stopPropagation()}>
            <div className="modal-header">
              <div className="modal-title">
                <Settings size={18} color="var(--lime)" />
                <span>PulseDL Preferences</span>
              </div>
              <button
                className="action-btn"
                onClick={() => setIsSettingsModalOpen(false)}
                style={{ padding: "4px" }}
              >
                <X size={16} />
              </button>
            </div>

            <div className="form-group">
              <label className="form-label">Default Download Directory</label>
              <div style={{ display: "flex", gap: "8px" }}>
                <input
                  type="text"
                  className="form-input"
                  value={settings.default_download_dir}
                  onChange={(e) =>
                    setSettings({ ...settings, default_download_dir: e.target.value })
                  }
                />
                <button
                  type="button"
                  className="btn btn-secondary"
                  onClick={async () => {
                    const sel = await invoke<string | null>("pick_folder");
                    if (sel) {
                      setSettings({ ...settings, default_download_dir: sel });
                    }
                  }}
                  style={{ whiteSpace: "nowrap" }}
                >
                  Browse...
                </button>
              </div>
            </div>

            <div className="form-group">
              <label className="form-label">Speed Limiter (Bytes/sec)</label>
              <select
                className="form-input"
                value={settings.speed_limit_bps}
                onChange={(e) =>
                  setSettings({ ...settings, speed_limit_bps: Number(e.target.value) })
                }
              >
                <option value={0}>⚡ Unlimited (99% Turbo Bandwidth)</option>
                <option value={2 * 1024 * 1024}>2 MB/s Limit</option>
                <option value={5 * 1024 * 1024}>5 MB/s Limit</option>
                <option value={10 * 1024 * 1024}>10 MB/s Limit</option>
                <option value={25 * 1024 * 1024}>25 MB/s Limit</option>
                <option value={50 * 1024 * 1024}>50 MB/s Limit</option>
              </select>
            </div>

            <div style={{ marginTop: "14px", display: "flex", flexDirection: "column", gap: "10px" }}>
              <label style={{ display: "flex", alignItems: "center", gap: "10px", fontSize: "13px", cursor: "pointer" }}>
                <input
                  type="checkbox"
                  checked={settings.autostart}
                  onChange={(e) => setSettings({ ...settings, autostart: e.target.checked })}
                />
                <span>Start silently in System Tray on Windows boot (No window popup)</span>
              </label>

              <label style={{ display: "flex", alignItems: "center", gap: "10px", fontSize: "13px", cursor: "pointer" }}>
                <input
                  type="checkbox"
                  checked={settings.minimize_to_tray}
                  onChange={(e) => setSettings({ ...settings, minimize_to_tray: e.target.checked })}
                />
                <span>Minimize to System Tray when closing the window</span>
              </label>

              <label style={{ display: "flex", alignItems: "center", gap: "10px", fontSize: "13px", cursor: "pointer" }}>
                <input
                  type="checkbox"
                  checked={settings.show_popup_on_download}
                  onChange={(e) =>
                    setSettings({ ...settings, show_popup_on_download: e.target.checked })
                  }
                />
                <span>Show separate floating popup window for each new download</span>
              </label>
            </div>

            <div
              style={{
                marginTop: "16px",
                padding: "12px",
                background: "#080e0a",
                border: "1px solid var(--border-color)",
                borderRadius: "6px",
              }}
            >
              <div style={{ fontSize: "12px", fontWeight: "700", color: "var(--lime)", marginBottom: "4px" }}>
                Browser Extension Setup:
              </div>
              <div style={{ fontSize: "11px", color: "var(--text-dim)", lineHeight: "1.5" }}>
                Open Chrome or Edge &gt; navigate to <code>chrome://extensions</code> &gt; Enable
                "Developer mode" &gt; Click "Load unpacked" &gt; Select folder:
                <br />
                <code style={{ color: "var(--lime)" }}>d:\faster download\extension</code>
              </div>
            </div>

            <div className="modal-actions">
              <button
                type="button"
                className="btn btn-secondary"
                onClick={() => setIsSettingsModalOpen(false)}
              >
                Cancel
              </button>
              <button type="button" className="btn btn-primary" onClick={handleSaveSettings}>
                Save Preferences
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
