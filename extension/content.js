// PulseDL Video & Download Grabber - Content Script
(function () {
  let floatingBtn = null;
  let menu = null;
  let activeVideo = null;
  let hideTimeout = null;
  let isHovered = false;
  let settings = {
    enableFloatingBtn: true,
    interceptDownloads: true
  };

  // Load extension settings
  if (chrome.runtime && chrome.storage && chrome.storage.local) {
    chrome.storage.local.get(["enableFloatingBtn", "interceptDownloads"], (res) => {
      if (res.enableFloatingBtn !== undefined) settings.enableFloatingBtn = res.enableFloatingBtn;
      if (res.interceptDownloads !== undefined) settings.interceptDownloads = res.interceptDownloads;
    });

    chrome.storage.onChanged.addListener((changes) => {
      if (changes.enableFloatingBtn) {
        settings.enableFloatingBtn = changes.enableFloatingBtn.newValue;
        if (!settings.enableFloatingBtn && floatingBtn) {
          floatingBtn.style.display = "none";
        }
      }
    });
  }

  function createFloatingButton() {
    if (floatingBtn) return floatingBtn;

    const container = document.createElement("div");
    container.id = "pulsedl-video-grabber-container";
    container.style.display = "none";

    container.innerHTML = `
      <div class="pulsedl-btn-bar" id="pulsedl-main-bar">
        <span class="pulsedl-play-icon">▶</span>
        <span class="pulsedl-btn-title">Download this video</span>
        <span class="pulsedl-separator"></span>
        <span class="pulsedl-dropdown-arrow" title="Download Options">▼</span>
        <span class="pulsedl-separator"></span>
        <span class="pulsedl-close-btn" title="Dismiss">✕</span>
      </div>
      <div class="pulsedl-menu" id="pulsedl-menu">
        <div class="pulsedl-menu-header">PulseDL Turbo Grabber</div>
        <div class="pulsedl-menu-item" data-quality="best">
          <span>⚡ Best Quality (Merged 4K/1080p)</span>
          <span class="pulsedl-badge">PRO</span>
        </div>
        <div class="pulsedl-menu-item" data-quality="1080">
          <span>🎬 1080p Full HD</span>
          <span class="pulsedl-badge">HD</span>
        </div>
        <div class="pulsedl-menu-item" data-quality="720">
          <span>🎬 720p HD</span>
          <span class="pulsedl-badge">FAST</span>
        </div>
        <div class="pulsedl-menu-item" data-quality="audio">
          <span>🎵 Audio Only (MP3/HQ)</span>
          <span class="pulsedl-badge">AUDIO</span>
        </div>
      </div>
    `;

    document.body.appendChild(container);
    floatingBtn = container;
    menu = container.querySelector("#pulsedl-menu");

    const mainBar = container.querySelector("#pulsedl-main-bar");
    const closeBtn = container.querySelector(".pulsedl-close-btn");

    mainBar.addEventListener("click", (e) => {
      if (e.target === closeBtn) return;
      e.stopPropagation();
      menu.classList.toggle("show");
    });

    closeBtn.addEventListener("click", (e) => {
      e.stopPropagation();
      container.style.display = "none";
      menu.classList.remove("show");
    });

    container.addEventListener("mouseenter", () => {
      isHovered = true;
      if (hideTimeout) clearTimeout(hideTimeout);
    });

    container.addEventListener("mouseleave", () => {
      isHovered = false;
      menu.classList.remove("show");
      resetHideTimer();
    });

    // Handle format selection clicks
    container.querySelectorAll(".pulsedl-menu-item").forEach((item) => {
      item.addEventListener("click", (e) => {
        e.stopPropagation();
        const quality = item.getAttribute("data-quality");
        menu.classList.remove("show");
        triggerDownload(quality);
      });
    });

    return container;
  }

  function resetHideTimer() {
    if (hideTimeout) clearTimeout(hideTimeout);
    hideTimeout = setTimeout(() => {
      if (!isHovered && floatingBtn) {
        floatingBtn.style.opacity = "0";
        setTimeout(() => {
          if (!isHovered && floatingBtn) floatingBtn.style.display = "none";
        }, 300);
      }
    }, 2800);
  }

  function showButtonForVideo(video) {
    if (!settings.enableFloatingBtn) return;
    const btn = createFloatingButton();
    activeVideo = video;

    const rect = video.getBoundingClientRect();
    if (rect.width < 120 || rect.height < 90) return; // Skip tiny video avatars/previews

    const scrollTop = window.pageYOffset || document.documentElement.scrollTop;
    const scrollLeft = window.pageXOffset || document.documentElement.scrollLeft;

    let top = rect.top + scrollTop + 12;
    let left = rect.left + scrollLeft + 12;

    btn.style.top = `${top}px`;
    btn.style.left = `${left}px`;
    btn.style.display = "block";
    btn.style.opacity = "1";

    resetHideTimer();
  }

  function findVideoUrl(video) {
    const pageUrl = window.location.href;

    // 1. Direct page permalinks
    if (
      pageUrl.includes("/watch?") ||
      pageUrl.includes("/watch/?v=") ||
      pageUrl.includes("watch?v=") ||
      pageUrl.includes("/reel/") ||
      pageUrl.includes("/videos/") ||
      pageUrl.includes("/shorts/") ||
      pageUrl.includes("youtu.be/") ||
      pageUrl.includes("tiktok.com/@")
    ) {
      return pageUrl;
    }

    if (!video) return pageUrl;

    // 2. Search ancestor elements for specific post / video permalinks
    let el = video.parentElement;
    for (let depth = 0; depth < 16 && el; depth++) {
      const links = el.querySelectorAll("a[href]");
      for (const link of links) {
        const href = link.href || "";
        if (
          href.includes("/watch/?v=") ||
          href.includes("/watch?v=") ||
          href.includes("/reel/") ||
          href.includes("/videos/") ||
          href.includes("fb.watch/") ||
          href.includes("/shorts/") ||
          href.includes("tiktok.com/@") ||
          href.includes("/p/") ||
          href.includes("/video/")
        ) {
          return href;
        }
      }
      el = el.parentElement;
    }

    // 3. Check direct video src
    if (video.currentSrc && !video.currentSrc.startsWith("blob:") && !video.currentSrc.startsWith("data:")) {
      return video.currentSrc;
    }
    if (video.src && !video.src.startsWith("blob:") && !video.src.startsWith("data:")) {
      return video.src;
    }

    return pageUrl;
  }

  function findVideoTitle(video) {
    if (!video) return document.title || "Video";

    // Try finding caption or heading in parent post
    let el = video.parentElement;
    for (let depth = 0; depth < 14 && el; depth++) {
      const heading = el.querySelector("h1, h2, h3, [role='heading'], div[dir='auto'][style*='webkit-line-clamp'], div[data-ad-preview='message']");
      if (heading && heading.textContent && heading.textContent.trim().length > 3) {
        return heading.textContent.trim().slice(0, 70);
      }
      el = el.parentElement;
    }

    const docTitle = document.title ? document.title.replace(/\|.*$/, "").replace(/-.*$/, "").trim() : "";
    if (docTitle && !["Facebook", "YouTube", "TikTok", "Instagram"].includes(docTitle)) {
      return docTitle;
    }

    const now = new Date();
    const timeStr = `${now.getFullYear()}${String(now.getMonth() + 1).padStart(2, "0")}${String(now.getDate()).padStart(2, "0")}_${String(now.getHours()).padStart(2, "0")}${String(now.getMinutes()).padStart(2, "0")}${String(now.getSeconds()).padStart(2, "0")}`;
    return `Facebook_Video_${timeStr}`;
  }

  function triggerDownload(quality) {
    const pageUrl = window.location.href;
    const finalUrl = findVideoUrl(activeVideo);
    const finalTitle = findVideoTitle(activeVideo);

    const isVideoSite =
      finalUrl.includes("youtube.com") ||
      finalUrl.includes("youtu.be") ||
      finalUrl.includes("facebook.com") ||
      finalUrl.includes("fb.watch") ||
      finalUrl.includes("tiktok.com") ||
      finalUrl.includes("instagram.com") ||
      finalUrl.includes("vimeo.com") ||
      pageUrl.includes("facebook.com") ||
      pageUrl.includes("youtube.com");

    const payload = {
      url: finalUrl,
      title: finalTitle,
      quality: quality || "best",
      is_video: isVideoSite || true,
      page_url: pageUrl
    };

    // Forward through background service worker to prevent Mixed Content blocking on HTTPS
    chrome.runtime.sendMessage(
      {
        type: "START_DOWNLOAD",
        payload: payload
      },
      (res) => {
        if (chrome.runtime.lastError) {
          showToast("⚠️ Could not reach PulseDL extension service worker: " + chrome.runtime.lastError.message);
          return;
        }

        if (res && res.success) {
          // Download successfully sent to PulseDL; Native floating popup handles UI
        } else {
          const err = res && res.error ? res.error : "Could not reach PulseDL. Please ensure PulseDL app is running!";
          showToast("⚠️ " + err);
        }
      }
    );
  }

  function showToast(text) {
    const existing = document.querySelector(".pulsedl-toast");
    if (existing) existing.remove();

    const toast = document.createElement("div");
    toast.className = "pulsedl-toast";
    toast.innerHTML = text;
    document.body.appendChild(toast);

    setTimeout(() => {
      toast.style.opacity = "0";
      toast.style.transition = "opacity 0.4s ease";
      setTimeout(() => toast.remove(), 400);
    }, 4000);
  }

  function scanVideos() {
    const videos = document.querySelectorAll("video");
    videos.forEach((video) => {
      if (video.dataset.pulsedlBound) return;
      video.dataset.pulsedlBound = "true";

      video.addEventListener("mouseenter", () => showButtonForVideo(video));
      video.addEventListener("play", () => showButtonForVideo(video));
      video.addEventListener("mousemove", () => {
        if (floatingBtn && floatingBtn.style.display === "none") {
          showButtonForVideo(video);
        } else {
          resetHideTimer();
        }
      });
    });
  }

  scanVideos();
  const observer = new MutationObserver(() => scanVideos());
  observer.observe(document.body, { childList: true, subtree: true });
})();
