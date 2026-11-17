// PulseDL Background Service Worker - Ultra Resilient & High-Speed
const PULSEDL_API = "http://127.0.0.1:45732/api";

// Initialize default settings
chrome.runtime.onInstalled.addListener(() => {
  chrome.storage.local.set({
    enableFloatingBtn: true,
    interceptDownloads: true
  });
});

const INTERCEPT_EXTENSIONS = [
  "zip", "rar", "7z", "tar", "gz", "xz", "bz2",
  "iso", "img", "bin", "dmg", "exe", "msi", "apk",
  "mp4", "mkv", "avi", "mov", "flv", "webm",
  "mp3", "flac", "wav", "aac", "ogg", "m4a",
  "pdf", "docx", "xlsx", "pptx", "csv", "json"
];

function shouldIntercept(filename, url) {
  if (!url) return false;
  // NEVER intercept blob or data URLs (e.g. ChatGPT canvas, local blobs)
  if (url.startsWith("blob:") || url.startsWith("data:")) return false;

  const target = (filename || url).toLowerCase().split("?")[0].split("#")[0];
  const parts = target.split(".");
  if (parts.length > 1) {
    const ext = parts.pop();
    return INTERCEPT_EXTENSIONS.includes(ext);
  }

  // Google Drive & Docs export links
  if (url.includes("docs.google.com") && url.includes("export?format=")) {
    return true;
  }
  if (url.includes("drive.google.com") || url.includes("drive.usercontent.google.com")) {
    return true;
  }

  return false;
}

// Extract cookies for authentication (Google Drive, Google Docs, ChatGPT, etc.)
async function getCookiesForUrl(url, referrer) {
  try {
    let cookies = await chrome.cookies.getAll({ url });
    if ((!cookies || cookies.length === 0) && referrer) {
      cookies = await chrome.cookies.getAll({ url: referrer });
    }
    if (cookies && cookies.length > 0) {
      return cookies.map((c) => `${c.name}=${c.value}`).join("; ");
    }
  } catch (e) {
    console.warn("[PulseDL] Cookie extraction error:", e);
  }
  return "";
}

// Send download request to PulseDL
async function sendToPulseDL(payload) {
  const controller = new AbortController();
  const timeoutId = setTimeout(() => controller.abort(), 12000);

  try {
    const response = await fetch(`${PULSEDL_API}/download`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(payload),
      signal: controller.signal
    });
    clearTimeout(timeoutId);

    if (response.ok) {
      const data = await response.json();
      return { success: true, data };
    }
    const err = await response.text();
    return { success: false, error: err };
  } catch (err) {
    clearTimeout(timeoutId);
    const isAbort = err.name === "AbortError" || (err.message && err.message.includes("abort"));
    return {
      success: false,
      error: isAbort
        ? "PulseDL is taking too long to respond. Please ensure PulseDL app is running!"
        : (err.message || "PulseDL Core not reachable")
    };
  }
}

// Intercept browser downloads
chrome.downloads.onDeterminingFilename.addListener((item, suggest) => {
  // Always skip blob/data URLs immediately
  if (!item.url || item.url.startsWith("blob:") || item.url.startsWith("data:")) {
    suggest();
    return;
  }

  chrome.storage.local.get(["interceptDownloads"], async (res) => {
    const interceptEnabled = res.interceptDownloads !== false;

    if (!interceptEnabled || !shouldIntercept(item.filename, item.url)) {
      suggest();
      return;
    }

    try {
      const cookies = await getCookiesForUrl(item.url, item.referrer);
      const payload = {
        url: item.url,
        filename: item.filename || null,
        referer: item.referrer || "",
        cookies: cookies || null,
        file_size: item.totalBytes > 0 ? item.totalBytes : null,
        is_video: false
      };

      // Try sending to PulseDL Core
      const result = await sendToPulseDL(payload);

      if (result.success) {
        // PulseDL successfully queued it, cancel native browser download
        chrome.downloads.cancel(item.id, () => {
          chrome.downloads.erase({ id: item.id });
        });
        showBadge("DL", "#00ff66");
      } else {
        // Fallback: If PulseDL is closed or fails, let browser download smoothly with zero error!
        suggest();
      }
    } catch (e) {
      suggest();
    }
  });

  return true;
});

function showBadge(text, color) {
  chrome.action.setBadgeText({ text });
  chrome.action.setBadgeBackgroundColor({ color });
  setTimeout(() => {
    chrome.action.setBadgeText({ text: "" });
  }, 4000);
}

// Handle messages from content script or popup
chrome.runtime.onMessage.addListener((request, sender, sendResponse) => {
  if (request.type === "START_DOWNLOAD") {
    (async () => {
      const cookies = await getCookiesForUrl(request.payload.url, request.payload.page_url);
      const fullPayload = {
        ...request.payload,
        cookies: cookies || null,
        referer: request.payload.page_url || ""
      };

      const result = await sendToPulseDL(fullPayload);
      sendResponse(result);
    })();
    return true; // asynchronous response
  }

  if (request.type === "CHECK_ENGINE") {
    fetch(`${PULSEDL_API}/ping`)
      .then((res) => res.json())
      .then((data) => sendResponse({ online: true, data }))
      .catch(() => sendResponse({ online: false }));
    return true;
  }
});
