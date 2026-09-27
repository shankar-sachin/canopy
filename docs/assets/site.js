// Canopy site: screenshot gallery, frame scaling, copy buttons, active nav.

// Gallery (home page): one tab per frame inside #screen.
const screen = document.getElementById("screen");
const tabs = document.getElementById("shot-tabs");
const title = document.getElementById("shot-title");
if (screen && tabs) {
  const shots = [...screen.querySelectorAll("pre.frame")];
  const show = id => {
    shots.forEach(f => (f.hidden = f.id !== id));
    [...tabs.children].forEach(b => b.setAttribute("aria-selected", String(b.dataset.id === id)));
    const f = document.getElementById(id);
    if (f && title) title.textContent = "canopy — " + f.dataset.label;
    fitAll();
  };
  shots.forEach(f => {
    const b = document.createElement("button");
    b.type = "button";
    b.setAttribute("role", "tab");
    b.dataset.id = f.id;
    b.textContent = f.dataset.label;
    b.addEventListener("click", () => show(f.id));
    tabs.appendChild(b);
  });
  if (shots.length) show(shots[0].id);
}

// Desktop screenshots (home page): one tab per image in #app-shots.
const appShots = document.getElementById("app-shots");
const appTabs = document.getElementById("app-tabs");
if (appShots && appTabs) {
  const imgs = [...appShots.querySelectorAll("img")];
  const show = img => {
    imgs.forEach(i => (i.hidden = i !== img));
    [...appTabs.children].forEach((b, k) => b.setAttribute("aria-selected", String(imgs[k] === img)));
  };
  imgs.forEach(img => {
    const b = document.createElement("button");
    b.type = "button";
    b.setAttribute("role", "tab");
    b.textContent = img.dataset.label;
    b.addEventListener("click", () => show(img));
    appTabs.appendChild(b);
  });
  if (imgs.length) show(imgs[0]);
}

// Desktop app / terminal app switch above the screenshots.
document.querySelectorAll(".surface-switch [data-surface]").forEach(btn => {
  btn.addEventListener("click", () => {
    const desktop = btn.dataset.surface === "desktop";
    btn.parentElement.querySelectorAll("[data-surface]").forEach(b => b.setAttribute("aria-selected", String(b === btn)));
    for (const id of ["app-shots", "app-tabs"]) document.getElementById(id).hidden = !desktop;
    for (const id of ["term-window", "shot-tabs"]) document.getElementById(id).hidden = desktop;
    fitAll();
  });
});

// Scale each visible terminal frame so its widest line fits its container.
function fitAll() {
  document.querySelectorAll(".screen").forEach(box => {
    const frame = [...box.querySelectorAll("pre.frame")].find(f => !f.hidden);
    if (!frame) return;
    const cols = Math.max(...frame.textContent.split("\n").map(l => l.length));
    const avail = box.clientWidth - 28;
    const size = Math.max(6.5, Math.min(13, avail / (cols * 0.602)));
    box.querySelectorAll("pre.frame").forEach(f => (f.style.fontSize = size + "px"));
  });
}
addEventListener("resize", fitAll);
if (document.fonts) document.fonts.ready.then(fitAll);
fitAll();

// ---------- Which system, and which releases ----------

/// The visitor's system: "mac", "windows", "linux", or "" (a phone or
/// tablet, or something else). iPads say they're Macs, so check for touch.
function visitorOS() {
  // ?os=mac|windows|linux shows the page as another system would see it.
  const forced = new URLSearchParams(location.search).get("os");
  if (forced === "mac" || forced === "windows" || forced === "linux") return forced;
  const ua = navigator.userAgent.toLowerCase();
  const p = ((navigator.userAgentData && navigator.userAgentData.platform) || navigator.platform || "").toLowerCase();
  if (/iphone|ipad|ipod|android/.test(ua)) return "";
  if (p.startsWith("mac") || ua.includes("mac os x")) return navigator.maxTouchPoints > 1 ? "" : "mac";
  if (p.startsWith("win") || ua.includes("windows")) return "windows";
  if (p.includes("linux") || ua.includes("linux") || ua.includes("x11")) return "linux";
  return "";
}

/// Canopy's releases from GitHub (newest first), kept for an hour so moving
/// between pages doesn't ask again. Each: { tag, date, url, assets: [{ name, url, size }] }.
let releasesPromise = null;
function canopyReleases() {
  if (releasesPromise) return releasesPromise;
  releasesPromise = (async () => {
    try {
      const cached = JSON.parse(sessionStorage.getItem("canopy-releases") || "null");
      if (cached && Date.now() - cached.at < 3600e3) return cached.list;
    } catch (e) {}
    const r = await fetch("https://api.github.com/repos/shankar-sachin/canopy/releases?per_page=100");
    if (!r.ok) throw new Error("GitHub answered " + r.status);
    const list = (await r.json())
      .filter((x) => !x.draft)
      .map((x) => ({
        tag: x.tag_name,
        date: x.published_at,
        pre: x.prerelease,
        url: x.html_url,
        assets: x.assets.map((a) => ({ name: a.name, url: a.browser_download_url, size: a.size })),
      }));
    try { sessionStorage.setItem("canopy-releases", JSON.stringify({ at: Date.now(), list })); } catch (e) {}
    return list;
  })();
  releasesPromise.catch(() => { releasesPromise = null; });
  return releasesPromise;
}

// Home page: the install command for this visitor's system.
const heroCmd = document.getElementById("brew-cmd");
const heroNote = document.getElementById("hero-os");
if (heroCmd && heroNote) {
  const os = visitorOS();
  if (os === "windows") {
    heroCmd.textContent = "winget install shankars.canopy-desktop";
    heroNote.textContent = "For Windows 11, with winget.";
  } else if (os === "linux") {
    heroNote.textContent = "For Debian and Ubuntu (x86_64).";
    heroCmd.textContent = "curl -LO https://github.com/shankar-sachin/canopy/releases/latest/…";
    canopyReleases().then((list) => {
      const rel = list.find((r) => !r.pre && r.assets.some((a) => a.name.endsWith("-amd64.deb")));
      const deb = rel && rel.assets.find((a) => a.name.endsWith("-amd64.deb"));
      if (!deb) throw new Error("no .deb");
      heroCmd.textContent = `curl -LO ${deb.url} && sudo apt install ./${deb.name}`;
    }).catch(() => {
      heroCmd.textContent = "https://github.com/shankar-sachin/canopy/releases/latest";
      heroNote.textContent = "For Linux: get the .deb or AppImage from the latest release.";
    });
  } else if (os === "mac") {
    heroNote.textContent = "For macOS, with Homebrew.";
  } else {
    heroNote.textContent = "Canopy runs on macOS, Windows and Linux.";
  }
}

// Copy buttons: data-copy-text, or data-copy="<element id>". One delegated
// handler, with a fallback for browsers that refuse the Clipboard API.
function copyText(text) {
  if (navigator.clipboard && window.isSecureContext) {
    return navigator.clipboard.writeText(text).then(() => true, () => legacyCopy(text));
  }
  return Promise.resolve(legacyCopy(text));
}
function legacyCopy(text) {
  const ta = document.createElement("textarea");
  ta.value = text;
  ta.setAttribute("readonly", "");
  ta.style.cssText = "position:fixed;top:0;left:0;opacity:0;pointer-events:none";
  document.body.append(ta);
  ta.select();
  let ok = false;
  try { ok = document.execCommand("copy"); } catch { ok = false; }
  ta.remove();
  return ok;
}
// Every code block gets a copy button. The block is wrapped so the button
// sits on the frame, not in the scrolling text, and stays put when a long
// command scrolls sideways.
const COPY_ICONS = '<svg class="i-copy" viewBox="0 0 16 16" width="15" height="15" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round"><rect x="5.5" y="5.5" width="8" height="8" rx="1.6"/><path d="M10.5 5.5V3.6A1.1 1.1 0 0 0 9.4 2.5H3.6a1.1 1.1 0 0 0-1.1 1.1v5.8a1.1 1.1 0 0 0 1.1 1.1h1.9"/></svg><svg class="i-done" viewBox="0 0 16 16" width="15" height="15" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="m3.5 8.5 3 3 6-7"/></svg>';

/// The commands in a block: no prompts ($, PS>), no comment lines.
function blockText(pre) {
  const clone = pre.cloneNode(true);
  clone.querySelectorAll(".copy, .p").forEach((el) => el.remove());
  clone.querySelectorAll(".m").forEach((el) => {
    // A comment on its own line goes; a trailing comment is cut off.
    el.remove();
  });
  return clone.textContent
    .split("\n")
    .map((l) => l.replace(/\s+$/, ""))
    .filter((l) => l.trim())
    .join("\n");
}

document.querySelectorAll("pre.code").forEach((pre) => {
  if (pre.parentElement.classList.contains("codebox")) return;
  const box = document.createElement("div");
  box.className = "codebox";
  pre.replaceWith(box);
  box.append(pre);
  let btn = pre.querySelector(".copy");
  if (!btn) {
    btn = document.createElement("button");
    btn.className = "copy";
    btn.type = "button";
    btn.innerHTML = COPY_ICONS;
  }
  btn.setAttribute("aria-label", "Copy");
  btn.title = "Copy";
  if (!btn.dataset.copyText && !btn.dataset.copy) btn._pre = pre;
  box.append(btn);
});

document.addEventListener("click", async (e) => {
  const btn = e.target.closest(".copy");
  if (!btn) return;
  e.preventDefault();
  const text = (btn.dataset.copyText || (btn._pre && blockText(btn._pre)) || document.getElementById(btn.dataset.copy)?.textContent || "").trim();
  if (!text) return;
  const ok = await copyText(text);
  btn.classList.toggle("done", ok);
  btn.setAttribute("aria-label", ok ? "Copied" : "Copy failed: select the text instead");
  btn.title = ok ? "Copied" : "Couldn't copy: select the text instead";
  clearTimeout(btn._t);
  btn._t = setTimeout(() => {
    btn.classList.remove("done");
    btn.setAttribute("aria-label", "Copy");
    btn.title = "Copy";
  }, 1600);
});

// Highlight the current page in the nav.
const here = location.pathname.split("/").pop() || "index.html";
document.querySelectorAll("nav.top .links a").forEach(a => {
  if (a.getAttribute("href") === here) a.setAttribute("aria-current", "page");
});

// ======================================================================
// Shared chrome: top bar menus, theme, and site-wide search.
(() => {
  const root = document.documentElement;
  const bar = document.querySelector(".topbar");
  if (!bar) return;
  const base = bar.dataset.root || "";
  const isMac = /Mac|iPhone|iPad/.test(navigator.userAgentData?.platform || navigator.platform || navigator.userAgent);
  document.querySelectorAll(".kbd-shortcut").forEach(k => (k.textContent = isMac ? k.dataset.mac : k.dataset.other));

  // ------------------------------------------------------------ menus
  const menus = [...bar.querySelectorAll(".menu")];
  const closeAll = except => menus.forEach(m => {
    if (m === except) return;
    m.classList.remove("open");
    m.querySelector("button").setAttribute("aria-expanded", "false");
  });
  const openMenu = m => {
    closeAll(m);
    m.classList.add("open");
    m.querySelector("button").setAttribute("aria-expanded", "true");
  };
  const hoverable = matchMedia("(hover: hover) and (min-width: 821px)");
  menus.forEach(m => {
    const btn = m.querySelector("button");
    btn.addEventListener("click", () => (m.classList.contains("open") ? closeAll() : openMenu(m)));
    let timer;
    m.addEventListener("mouseenter", () => { if (hoverable.matches) { clearTimeout(timer); openMenu(m); } });
    m.addEventListener("mouseleave", () => { if (hoverable.matches) timer = setTimeout(() => closeAll(), 140); });
  });
  document.addEventListener("click", e => { if (!e.target.closest(".menu")) closeAll(); });

  const menuBtn = bar.querySelector(".menu-btn");
  menuBtn?.addEventListener("click", () => {
    const open = document.body.classList.toggle("menus-open");
    menuBtn.setAttribute("aria-expanded", String(open));
  });

  // ------------------------------------------------------------ theme
  const themeBtn = bar.querySelector(".theme-btn");
  const modes = ["system", "dark", "light"];
  const names = { system: "System", dark: "Dark", light: "Light" };
  const load = () => { try { return localStorage.getItem("canopy-theme"); } catch { return null; } };
  const save = v => { try { localStorage.setItem("canopy-theme", v); } catch { /* not remembered in private mode */ } };
  const apply = mode => {
    if (mode === "system") delete root.dataset.theme; else root.dataset.theme = mode;
    themeBtn.dataset.mode = mode;
    themeBtn.querySelector(".theme-label").textContent = names[mode];
    themeBtn.setAttribute("aria-label", `Color theme: ${names[mode]}. Click to change.`);
  };
  let mode = modes.includes(load()) ? load() : "system";
  if (themeBtn) {
    apply(mode);
    themeBtn.addEventListener("click", () => { mode = modes[(modes.indexOf(mode) + 1) % 3]; save(mode); apply(mode); });
  }

  // ----------------------------------------------------------- search
  let dialog, input, list, items = [], sel = 0;
  const esc = s => s.replace(/[&<>"]/g, c => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);
  const hl = (text, terms) => terms.reduce((out, t) =>
    out.replace(new RegExp(`(${t.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")})`, "ig"), "<mark>$1</mark>"), esc(text));

  function loadIndex() {
    return new Promise(resolve => {
      if (window.CANOPY_SEARCH) return resolve(window.CANOPY_SEARCH);
      const s = document.createElement("script");
      s.src = base + "assets/search-index.js";
      s.onload = () => resolve(window.CANOPY_SEARCH || []);
      s.onerror = () => resolve([]);
      document.head.appendChild(s);
    });
  }
  function build() {
    dialog = document.createElement("div");
    dialog.className = "search-dialog";
    dialog.hidden = true;
    dialog.setAttribute("role", "dialog");
    dialog.setAttribute("aria-modal", "true");
    dialog.setAttribute("aria-label", "Search Canopy");
    dialog.innerHTML = `<div class="search-box">
      <div class="search-input"><svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" aria-hidden="true"><circle cx="11" cy="11" r="7"/><path d="m20 20-3.5-3.5"/></svg>
      <input id="site-search" type="search" placeholder="Search the site and the wiki" autocomplete="off" spellcheck="false"><kbd>esc</kbd></div>
      <ul class="search-results" role="listbox"></ul>
      <div class="search-foot"><span><kbd>↑</kbd><kbd>↓</kbd> move</span><span><kbd>↵</kbd> open</span><span><kbd>esc</kbd> close</span></div></div>`;
    document.body.appendChild(dialog);
    input = dialog.querySelector("input");
    list = dialog.querySelector(".search-results");
    dialog.addEventListener("click", e => { if (e.target === dialog) close(); });
    input.addEventListener("input", render);
    input.addEventListener("keydown", e => {
      if (e.key === "ArrowDown") { e.preventDefault(); move(1); }
      else if (e.key === "ArrowUp") { e.preventDefault(); move(-1); }
      else if (e.key === "Enter") { e.preventDefault(); const a = list.querySelectorAll("li[role=option] a")[sel]; if (a) location.href = a.href; }
    });
  }
  function snippet(text, terms) {
    const lower = text.toLowerCase();
    const hits = terms.map(t => lower.indexOf(t)).filter(i => i >= 0);
    if (!hits.length) return "";
    const start = Math.max(0, Math.min(...hits) - 40);
    return (start ? "…" : "") + text.slice(start, start + 140) + "…";
  }
  function search(q, index) {
    const terms = q.toLowerCase().split(/\s+/).filter(Boolean);
    if (!terms.length) return index.map(p => ({ url: p.url, title: p.title, section: p.section, excerpt: p.lede }));
    const out = [];
    for (const p of index) {
      const hay = `${p.title} ${p.lede} ${p.headings.map(h => h.text).join(" ")} ${p.text}`.toLowerCase();
      if (!terms.every(t => hay.includes(t))) continue;
      let score = 0;
      for (const t of terms) {
        if (p.title.toLowerCase().includes(t)) score += 10;
        if (p.lede.toLowerCase().includes(t)) score += 3;
      }
      if (p.url.startsWith("wiki/")) score += 1;
      out.push({ url: p.url, title: p.title, section: p.section, excerpt: snippet(p.text, terms), score });
      for (const h of p.headings) {
        const ht = h.text.toLowerCase();
        if (terms.some(t => ht.includes(t))) out.push({ url: `${p.url}#${h.id}`, title: h.text, section: p.title, excerpt: "", score: score + 5 });
      }
    }
    return out.sort((a, b) => b.score - a.score).slice(0, 12);
  }
  async function render() {
    const index = await loadIndex();
    const q = input.value.trim();
    const terms = q.toLowerCase().split(/\s+/).filter(Boolean);
    items = search(q, index);
    sel = 0;
    list.innerHTML = items.length
      ? items.map((r, i) => `<li role="option" aria-selected="${i === 0}"><a href="${base}${r.url}"><span class="t">${hl(r.title, terms)}</span><span class="s">${esc(r.section)}</span>${r.excerpt ? `<span class="x">${hl(r.excerpt, terms)}</span>` : ""}</a></li>`).join("")
      : `<li class="empty">Nothing matches “${esc(q)}”.</li>`;
  }
  function move(d) {
    const lis = [...list.querySelectorAll("li[role=option]")];
    if (!lis.length) return;
    sel = (sel + d + lis.length) % lis.length;
    lis.forEach((li, i) => li.setAttribute("aria-selected", String(i === sel)));
    lis[sel].scrollIntoView({ block: "nearest" });
  }
  function open() {
    if (!dialog) build();
    closeAll();
    dialog.hidden = false;
    input.value = "";
    render();
    input.focus();
  }
  function close() { if (dialog) dialog.hidden = true; }
  bar.querySelector(".search-btn")?.addEventListener("click", open);
  addEventListener("keydown", e => {
    const typing = /INPUT|TEXTAREA|SELECT/.test(document.activeElement?.tagName || "");
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") { e.preventDefault(); dialog && !dialog.hidden ? close() : open(); }
    else if (e.key === "/" && !typing && (!dialog || dialog.hidden)) { e.preventDefault(); open(); }
    else if (e.key === "Escape") { if (dialog && !dialog.hidden) close(); closeAll(); }
  });
})();
