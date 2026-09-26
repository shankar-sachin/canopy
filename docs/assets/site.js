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

// Copy buttons: data-copy-text, or data-copy="<element id>".
document.querySelectorAll(".copy").forEach(btn => {
  btn.addEventListener("click", async () => {
    const text = btn.dataset.copyText || document.getElementById(btn.dataset.copy)?.textContent || "";
    try {
      await navigator.clipboard.writeText(text.trim());
      btn.textContent = "Copied";
      btn.classList.add("done");
      setTimeout(() => {
        btn.textContent = "Copy";
        btn.classList.remove("done");
      }, 1600);
    } catch {
      /* Clipboard blocked: the text stays selectable. */
    }
  });
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
