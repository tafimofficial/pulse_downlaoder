// PulseDL Extension Popup Script
document.addEventListener("DOMContentLoaded", () => {
  const toggleFloating = document.getElementById("toggle-floating-btn");
  const toggleIntercept = document.getElementById("toggle-intercept");
  const engineStatus = document.getElementById("engine-status");
  const statusText = document.getElementById("status-text");
  const btnDownloadTab = document.getElementById("btn-download-tab");

  // Load preferences
  chrome.storage.local.get(["enableFloatingBtn", "interceptDownloads"], (res) => {
    if (res.enableFloatingBtn !== undefined) toggleFloating.checked = res.enableFloatingBtn;
    if (res.interceptDownloads !== undefined) toggleIntercept.checked = res.interceptDownloads;
  });

  // Toggle events
  toggleFloating.addEventListener("change", () => {
    chrome.storage.local.set({ enableFloatingBtn: toggleFloating.checked });
  });

  toggleIntercept.addEventListener("change", () => {
    chrome.storage.local.set({ interceptDownloads: toggleIntercept.checked });
  });

  // Check backend engine connectivity
  function checkEngine() {
    fetch("http://127.0.0.1:45732/api/ping")
      .then((res) => res.json())
      .then((data) => {
        engineStatus.classList.add("online");
        statusText.textContent = "Turbo Core: Online";
      })
      .catch(() => {
        engineStatus.classList.remove("online");
        statusText.textContent = "Core Offline";
      });
  }

  checkEngine();

  // Download current page video
  btnDownloadTab.addEventListener("click", () => {
    chrome.tabs.query({ active: true, currentWindow: true }, (tabs) => {
      if (!tabs || !tabs[0]) return;
      const tab = tabs[0];

      fetch("http://127.0.0.1:45732/api/download", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          url: tab.url,
          title: tab.title || "Video Download",
          quality: "best",
          is_video: true
        })
      })
        .then((res) => res.json())
        .then(() => {
          btnDownloadTab.textContent = "✓ Sent to PulseDL!";
          setTimeout(() => {
            btnDownloadTab.innerHTML = "<span>⚡</span><span>Download Current Page Video</span>";
          }, 2500);
        })
        .catch(() => {
          btnDownloadTab.textContent = "⚠️ PulseDL Not Running";
          setTimeout(() => {
            btnDownloadTab.innerHTML = "<span>⚡</span><span>Download Current Page Video</span>";
          }, 2500);
        });
    });
  });
});
