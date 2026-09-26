// Canopy wiki: theme switcher, mobile menu, on-this-page, and search.
(() => {
  const root = document.documentElement;
  const isMac = /Mac|iPhone|iPad/.test(navigator.userAgentData?.platform || navigator.platform || navigator.userAgent);

  // ⌘ K on Macs, Ctrl K everywhere else.
  document.querySelectorAll(".kbd-shortcut").forEach(k => (k.textContent = isMac ? k.dataset.mac : k.dataset.other));

  // ---------------------------------------------------------------- theme
  const themeBtn = document.querySelector(".wtheme");
  const modes = ["system", "dark", "light"];
  const label = { system: "System", dark: "Dark", light: "Light" };
  const store = {
    get() { try { return localStorage.getItem("canopy-theme"); } catch { return null; } },
    set(v) { try { localStorage.setItem("canopy-theme", v); } catch { /* private mode: not remembered */ } },
  };
  function applyTheme(mode) {
    if (mode === "system") delete root.dataset.theme; else root.dataset.theme = mode;
    if (themeBtn) {
      themeBtn.dataset.mode = mode;
      themeBtn.querySelector(".wtheme-label").textContent = label[mode];
      themeBtn.setAttribute("aria-label", `Color theme: ${label[mode]}. Click to change.`);
    }
  }
  let mode = modes.includes(store.get()) ? store.get() : "system";
  applyTheme(mode);
  themeBtn?.addEventListener("click", () => {
    mode = modes[(modes.indexOf(mode) + 1) % modes.length];
    store.set(mode);
    applyTheme(mode);
  });

  // ---------------------------------------------------------- mobile menu
  const menuBtn = document.querySelector(".wmenu");
  menuBtn?.addEventListener("click", () => {
    const open = document.body.classList.toggle("nav-open");
    menuBtn.setAttribute("aria-expanded", String(open));
  });
  document.querySelector(".wprose")?.addEventListener("click", () => {
    document.body.classList.remove("nav-open");
    menuBtn?.setAttribute("aria-expanded", "false");
  });

  // ------------------------------------------------------- on this page
  const toc = document.getElementById("toc");
  const heads = [...document.querySelectorAll(".wprose h2[id], .wprose h3[id]")];
  if (toc) {
    if (!heads.length) {
      toc.closest(".wtoc").style.visibility = "hidden";
    }
    heads.forEach(h => {
      const a = document.createElement("a");
      a.href = "#" + h.id;
      a.textContent = h.textContent.replace(/#$/, "").trim();
      if (h.tagName === "H3") a.className = "sub";
      toc.appendChild(a);
    });
    const links = [...toc.querySelectorAll("a")];
    const setActive = () => {
      const top = 120;
      let current = heads[0];
      for (const h of heads) if (h.getBoundingClientRect().top <= top) current = h;
      links.forEach(l => l.classList.toggle("active", current && l.getAttribute("href") === "#" + current.id));
    };
    addEventListener("scroll", setActive, { passive: true });
    setActive();
  }

  // --------------------------------------------------------------- search
  const dialog = document.querySelector(".wsearch-dialog");
  const input = document.getElementById("wsearch-q");
  const results = document.getElementById("wsearch-results");
  const index = window.WIKI_INDEX || [];
  let items = [];
  let sel = 0;

  const escapeHtml = s => s.replace(/[&<>"]/g, c => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);
  const mark = (text, terms) => {
    let out = escapeHtml(text);
    terms.forEach(t => { out = out.replace(new RegExp(`(${t.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")})`, "ig"), "<mark>$1</mark>"); });
    return out;
  };
  function snippet(text, terms) {
    const lower = text.toLowerCase();
    const at = Math.min(...terms.map(t => lower.indexOf(t)).filter(i => i >= 0));
    if (!isFinite(at)) return text.slice(0, 120);
    const start = Math.max(0, at - 40);
    return (start > 0 ? "…" : "") + text.slice(start, start + 140) + "…";
  }

  // Score pages and headings: title > heading > body text; every term must match.
  function search(q) {
    const terms = q.toLowerCase().split(/\s+/).filter(Boolean);
    if (!terms.length) {
      return index.map(p => ({ url: p.url, title: p.title, section: p.section, excerpt: p.lede, score: 0 }));
    }
    const found = [];
    for (const p of index) {
      const hay = `${p.title} ${p.lede} ${p.headings.map(h => h.text).join(" ")} ${p.text}`.toLowerCase();
      if (!terms.every(t => hay.includes(t))) continue;
      let score = 0;
      for (const t of terms) {
        if (p.title.toLowerCase().includes(t)) score += 10;
        if (p.lede.toLowerCase().includes(t)) score += 3;
      }
      found.push({ url: p.url, title: p.title, section: p.section, excerpt: snippet(p.text, terms), score });
      for (const h of p.headings) {
        const ht = h.text.toLowerCase();
        if (terms.every(t => ht.includes(t) || p.title.toLowerCase().includes(t)) && terms.some(t => ht.includes(t))) {
          found.push({ url: `${p.url}#${h.id}`, title: h.text, section: p.title, excerpt: "", score: score + 6 });
        }
      }
    }
    return found.sort((a, b) => b.score - a.score).slice(0, 12);
  }

  function render() {
    const q = input.value.trim();
    const terms = q.toLowerCase().split(/\s+/).filter(Boolean);
    items = search(q);
    sel = 0;
    results.innerHTML = items.length
      ? items.map((r, i) => `<li role="option" aria-selected="${i === 0}"><a href="${r.url}"><span class="t">${mark(r.title, terms)}</span><span class="s">${escapeHtml(r.section)}</span>${r.excerpt ? `<span class="x">${mark(r.excerpt, terms)}</span>` : ""}</a></li>`).join("")
      : `<li class="empty">No pages match “${escapeHtml(q)}”.</li>`;
  }
  function move(d) {
    const lis = [...results.querySelectorAll("li[role=option]")];
    if (!lis.length) return;
    sel = (sel + d + lis.length) % lis.length;
    lis.forEach((li, i) => li.setAttribute("aria-selected", String(i === sel)));
    lis[sel].scrollIntoView({ block: "nearest" });
  }
  function open() {
    if (!dialog) return;
    dialog.hidden = false;
    input.value = "";
    render();
    input.focus();
  }
  function close() { if (dialog) dialog.hidden = true; }

  document.querySelector(".wsearch")?.addEventListener("click", open);
  dialog?.addEventListener("click", e => { if (e.target === dialog) close(); });
  input?.addEventListener("input", render);
  input?.addEventListener("keydown", e => {
    if (e.key === "ArrowDown") { e.preventDefault(); move(1); }
    else if (e.key === "ArrowUp") { e.preventDefault(); move(-1); }
    else if (e.key === "Enter") { e.preventDefault(); const a = results.querySelectorAll("li[role=option] a")[sel]; if (a) location.href = a.href; }
  });
  addEventListener("keydown", e => {
    const typing = /INPUT|TEXTAREA|SELECT/.test(document.activeElement?.tagName || "");
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") { e.preventDefault(); dialog?.hidden ? open() : close(); }
    else if (e.key === "/" && !typing && dialog?.hidden) { e.preventDefault(); open(); }
    else if (e.key === "Escape" && dialog && !dialog.hidden) { close(); }
  });
})();
