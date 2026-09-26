// Canopy Desktop front end. Plain JS, no build step: talks to the Rust side
// through window.__TAURI__.core.invoke (see src/main.rs for the commands).
"use strict";

const invoke = (cmd, args) => window.__TAURI__.core.invoke(cmd, args);
const $ = (sel) => document.querySelector(sel);

const state = {
  overview: null,
  page: "home",
  busy: false,
};

// ---------------------------------------------------------------- helpers

function esc(s) {
  return String(s ?? "").replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]);
}

function ago(unix) {
  if (!unix) return "";
  const s = Math.max(0, Date.now() / 1000 - unix);
  const units = [["y", 31536000], ["mo", 2592000], ["w", 604800], ["d", 86400], ["h", 3600], ["m", 60]];
  for (const [u, n] of units) if (s >= n) return `${Math.floor(s / n)}${u} ago`;
  return "just now";
}

function plural(n, word) {
  return `${n} ${word}${n === 1 ? "" : "s"}`;
}

function toast(text, { error = false, detail = "" } = {}) {
  const el = document.createElement("div");
  el.className = "toast" + (error ? " error" : "");
  el.innerHTML = (error ? "<b>Something went wrong.</b> " : "") + esc(text) + (detail ? `<code>${esc(detail)}</code>` : "");
  $("#toasts").append(el);
  setTimeout(() => el.remove(), error ? 7000 : 3500);
}

const ICONS = {
  home: '<path d="M2.5 7.5 8 3l5.5 4.5V13a.5.5 0 0 1-.5.5H10v-4H6v4H3a.5.5 0 0 1-.5-.5z"/>',
  folder: '<path d="M2 4.5A1.5 1.5 0 0 1 3.5 3h2.8l1.4 1.5h4.8A1.5 1.5 0 0 1 14 6v5.5a1.5 1.5 0 0 1-1.5 1.5h-9A1.5 1.5 0 0 1 2 11.5z"/>',
  x: '<path d="M4 4l8 8M12 4l-8 8"/>',
  changes: '<path d="M4 2.5h5.5L12 5v8.5H4z"/><path d="M8 6.5v4M6 8.5h4"/>',
};

function icon(name, size = 16) {
  return `<svg viewBox="0 0 16 16" width="${size}" height="${size}" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">${ICONS[name]}</svg>`;
}

// ---------------------------------------------------------------- theme

function applyTheme(choice) {
  if (choice === "light" || choice === "dark") document.documentElement.dataset.theme = choice;
  else delete document.documentElement.dataset.theme;
  for (const b of document.querySelectorAll("[data-theme-choice]")) b.classList.toggle("on", b.dataset.themeChoice === choice);
  try { localStorage.setItem("canopy-theme", choice); } catch {}
}

function savedTheme() {
  try { return localStorage.getItem("canopy-theme") || "system"; } catch { return "system"; }
}

// ---------------------------------------------------------------- pages

// Each page: nav label, icon, render(overview) -> html.
const PAGES = {
  home: { title: "Home", icon: "home", render: renderHome },
};
const NAV = [{ items: ["home"] }];

/// Pages in other files add themselves here.
function addPage(id, page, group = 0) {
  PAGES[id] = page;
  NAV[group].items.push(id);
}

function renderNav() {
  const o = state.overview;
  let html = "";
  for (const group of NAV) {
    if (group.title) html += `<div class="nav-group">${esc(group.title)}</div>`;
    for (const id of group.items) {
      const p = PAGES[id];
      const badge = p.badge ? p.badge(o) : "";
      html += `<button class="nav-item${state.page === id ? " active" : ""}" data-page="${id}" type="button">${icon(p.icon)}<span>${esc(p.title)}</span>${badge}</button>`;
    }
  }
  $("#nav").innerHTML = html;
}

function syncPills(b) {
  if (!b.head) return `<span class="pill amber">detached</span>`;
  if (!b.upstream) return `<span class="pill">not published</span>`;
  if (!b.ahead && !b.behind) return `<span class="pill green">✓ in sync</span>`;
  let out = "";
  if (b.ahead) out += `<span class="pill teal" title="Commits to push">↑ <b>${b.ahead}</b></span>`;
  if (b.behind) out += `<span class="pill amber" title="Commits to pull">↓ <b>${b.behind}</b></span>`;
  return out;
}

const STATE_WORDS = { Merging: "Merging", Rebasing: "Rebasing", CherryPicking: "Cherry-picking", Reverting: "Reverting", Bisecting: "Bisecting" };

function renderHome(o) {
  const b = o.status.branch;
  const c = o.counts;
  const tiles = [
    ["Staged", c.staged, c.staged ? "green" : "dim"],
    ["Changed", c.unstaged, c.unstaged ? "amber" : "dim"],
    ["Conflicts", c.conflicts, c.conflicts ? "red" : "dim"],
    ["Stashes", o.stashes, o.stashes ? "" : "dim"],
    ["Branches", o.branches.filter((x) => !x.is_remote).length, ""],
    ["Remotes", o.remotes.length, o.remotes.length ? "" : "dim"],
  ];
  const steps = o.next_steps
    .map((s) => {
      const btn = s.action && ACTIONS[s.action] ? `<button class="btn small" data-action="${s.action}" type="button">${esc(ACTIONS[s.action].label)}</button>` : "";
      return `<li class="step ${s.level}"><span class="dot"></span><p>${esc(s.text)}</p>${btn}</li>`;
    })
    .join("");
  const commits = o.log.length
    ? `<ul class="commits">${o.log.slice(0, 12).map(commitRow).join("")}</ul>`
    : `<div class="empty">No commits yet.</div>`;
  const locals = o.branches.filter((x) => !x.is_remote).sort((x, y) => y.is_head - x.is_head || y.time - x.time);
  const branches = locals.length
    ? `<ul class="branches">${locals.slice(0, 10).map(branchRow).join("")}</ul>`
    : `<div class="empty">No branches yet.</div>`;
  const stateBadge = o.state !== "Clean" ? `<span class="pill red">${STATE_WORDS[o.state] || o.state}</span>` : "";
  const upstream = b.upstream ? `<span class="pill">tracks <b>${esc(b.upstream)}</b></span>` : "";

  return `<div class="grid">
    <div class="card hero span-12">
      <img class="hero-logo" src="logo.svg" alt="" width="52" height="52">
      <div class="hero-main">
        <h1>${esc(o.name)}</h1>
        <div class="hero-path selectable">${esc(o.root)}</div>
        <div class="hero-meta">
          <span class="pill">on <b>${esc(b.head || (b.oid || "").slice(0, 7) || "—")}</b></span>
          ${upstream}${syncPills(b)}${stateBadge}
        </div>
      </div>
    </div>
    <div class="tiles span-12">${tiles.map(([l, n, cls]) => `<div class="tile ${cls}"><div class="n">${n}</div><div class="l">${l}</div></div>`).join("")}</div>
    <div class="card span-12">
      <div class="card-h"><h3>Next steps</h3></div>
      <ul class="steps">${steps}</ul>
    </div>
    <div class="card span-7">
      <div class="card-h"><h3>Recent commits</h3><span class="sub">${o.log.length ? esc(o.log[0].author) + " · " + ago(o.log[0].time) : ""}</span></div>
      ${commits}
    </div>
    <div class="card span-5">
      <div class="card-h"><h3>Branches</h3><span class="sub">${locals.length} local branch${locals.length === 1 ? "" : "es"}</span></div>
      ${branches}
    </div>
  </div>`;
}

function refChip(r) {
  if (r.startsWith("HEAD -> ")) return `<span class="ref head">${esc(r.slice(8))}</span>`;
  if (r === "HEAD") return "";
  if (r.startsWith("tag: ")) return `<span class="ref tag">${esc(r.slice(5))}</span>`;
  if (r.includes("/")) return `<span class="ref remote">${esc(r)}</span>`;
  return `<span class="ref head">${esc(r)}</span>`;
}

// At most two labels, so the commit message stays readable.
function refChips(refs) {
  const shown = refs.filter((r) => r !== "HEAD");
  const more = shown.length > 2 ? `<span class="ref more" title="${esc(shown.slice(2).join(", "))}">+${shown.length - 2}</span>` : "";
  return shown.slice(0, 2).map(refChip).join("") + more;
}

function commitRow(c) {
  const merge = c.parents.length > 1 ? " merge" : "";
  return `<li class="commit${merge}" title="${esc(c.oid)}">
    <span class="lane"></span>
    <span class="oid">${esc(c.short)}</span>
    <span class="subj">${refChips(c.refs)}${esc(c.subject)}</span>
    <span class="meta">${esc(c.author)} · ${ago(c.time)}</span>
  </li>`;
}

function branchRow(br) {
  let track = "";
  if (br.track === "gone") track = `<span class="pill red">upstream gone</span>`;
  else if (br.track) {
    const a = /ahead (\d+)/.exec(br.track);
    const bh = /behind (\d+)/.exec(br.track);
    if (a) track += `<span class="pill teal">↑ <b>${a[1]}</b></span>`;
    if (bh) track += `<span class="pill amber">↓ <b>${bh[1]}</b></span>`;
  }
  return `<li class="branch${br.is_head ? " head" : ""}" title="${esc(br.subject)}">
    <span class="name">${br.is_head ? "● " : ""}${esc(br.name)}</span>${track}<span class="when">${ago(br.time)}</span>
  </li>`;
}

// Buttons on "next steps". Pages that don't exist yet have no button.
const ACTIONS = {
  push: { label: "Push", run: () => sync("push") },
  pull: { label: "Pull", run: () => sync("pull") },
};

// ---------------------------------------------------------------- render

function render() {
  const o = state.overview;
  if (!o) return;
  const b = o.status.branch;
  $("#repo-name").textContent = o.name;
  $("#repo-branch").textContent = b.head || "detached HEAD";
  $("#page-title").textContent = PAGES[state.page].title;
  $("#sync").innerHTML = syncPills(b);
  document.title = `${o.name} · Canopy`;
  renderNav();
  const view = $("#view");
  const page = PAGES[state.page];
  // Pages with their own state (like a half-written commit message) update
  // in place instead of being redrawn.
  if (page.update && view.dataset.page === state.page) {
    page.update(o, view);
    return;
  }
  const top = view.dataset.page === state.page ? view.scrollTop : 0;
  view.className = "view" + (page.full ? " full" : "");
  view.innerHTML = page.render(o);
  view.dataset.page = state.page;
  view.scrollTop = top;
  page.mounted?.(o, view);
}

function go(page) {
  if (!PAGES[page]) return;
  state.page = page;
  delete $("#view").dataset.page;
  render();
}

// ---------------------------------------------------------------- running git

/// Run a command that changes the repo, show what git ran, then refresh.
/// Resolves to the result, or null if it failed (the error is shown).
async function run(label, cmd, args) {
  state.busy = true;
  document.body.classList.add("busy");
  try {
    const res = await invoke(cmd, args);
    toast(label, { detail: res?.cmd });
    return res;
  } catch (e) {
    toast(label + " failed", { error: true, detail: String(e) });
    return null;
  } finally {
    state.busy = false;
    document.body.classList.remove("busy");
    await refresh({ quiet: true });
  }
}

/// A small dialog. buttons: [{label, value, kind: "primary" | "danger"}].
/// Resolves to the chosen value, or null for Cancel / Escape.
function ask({ title, text = "", html = "", buttons }) {
  return new Promise((resolve) => {
    const wrap = document.createElement("div");
    wrap.className = "modal-wrap";
    wrap.innerHTML = `<div class="modal" role="dialog" aria-modal="true">
      <h3>${esc(title)}</h3>${text ? `<p>${esc(text)}</p>` : ""}${html}
      <div class="modal-actions"><button class="btn ghost" data-v="" type="button">Cancel</button>${buttons
        .map((b, i) => `<button class="btn ${b.kind || ""}" data-v="${i}" type="button">${esc(b.label)}</button>`)
        .join("")}</div></div>`;
    const close = (v) => {
      wrap.remove();
      document.removeEventListener("keydown", onKey, true);
      resolve(v === "" || v == null ? null : buttons[Number(v)].value);
    };
    const onKey = (e) => {
      if (e.key === "Escape") { e.stopPropagation(); close(null); }
    };
    wrap.addEventListener("click", (e) => {
      if (e.target === wrap) close(null);
      const b = e.target.closest("[data-v]");
      if (b) close(b.dataset.v);
    });
    document.addEventListener("keydown", onKey, true);
    document.body.append(wrap);
    (wrap.querySelector(".btn.primary, .btn.danger") || wrap.querySelector(".btn")).focus();
  });
}

// ---------------------------------------------------------------- fetch / pull / push

const SYNC_LABELS = {
  fetch: "Fetch", pull: "Pull", "pull-rebase": "Pull (rebase)", "pull-merge": "Pull (merge)",
  push: "Push", "force-push": "Force push",
};

async function sync(kind) {
  const o = state.overview;
  if (!o || state.busy) return;
  const b = o.status.branch;
  if (kind === "push" && b.upstream && b.ahead && b.behind) {
    kind = await ask({
      title: `Your branch has diverged (${b.ahead} ahead, ${b.behind} behind)`,
      text: "The remote has commits you don't have. Bring them in first, then push again.",
      buttons: [
        { label: "Pull with merge", value: "pull-merge" },
        { label: "Pull with rebase", value: "pull-rebase", kind: "primary" },
        { label: "Force push", value: "force-push", kind: "danger" },
      ],
    });
    if (!kind) return;
    if (kind === "force-push" && !(await ask({
      title: "Force push?",
      text: `This replaces ${b.upstream} with your branch. The ${b.behind} commit(s) only on the remote will be lost (--force-with-lease stops if someone pushed since your last fetch).`,
      buttons: [{ label: "Force push", value: true, kind: "danger" }],
    }))) return;
  } else if (kind === "push" && b.upstream && !b.ahead) {
    toast(b.behind ? "Nothing to push: pull first." : "Nothing to push: already up to date.");
    return;
  } else if (kind === "pull" && !b.upstream) {
    toast("This branch isn't on the remote yet, so there's nothing to pull. Push it first.");
    return;
  }
  const bar = $("#progress");
  bar.hidden = false;
  bar.querySelector("b").textContent = SYNC_LABELS[kind] + "…";
  bar.querySelector("span").textContent = "";
  const res = await run(SYNC_LABELS[kind], "sync", { kind });
  bar.hidden = true;
  if (res?.output && kind !== "fetch") toast(res.output.split("\n").slice(-2).join(" "));
}

function onProgress(line) {
  const bar = $("#progress");
  if (!bar.hidden) bar.querySelector("span").textContent = line;
}

// ---------------------------------------------------------------- repos

async function showWelcome() {
  state.overview = null;
  document.title = "Canopy";
  $("#shell").hidden = true;
  $("#welcome").hidden = false;
  const repos = await invoke("recent_repos");
  $("#recent").innerHTML = repos.length
    ? `<h3>Recent</h3><div class="recent-list">${repos
        .map(
          (r) => `<div class="recent-item${r.exists ? "" : " missing"}" data-path="${esc(r.path)}" title="${r.exists ? "" : "This folder no longer exists"}">
            <span class="folder">${icon("folder")}</span>
            <span class="names"><b>${esc(r.name)}</b><small>${esc(r.display)}</small></span>
            <button class="btn icon ghost forget" data-forget="${esc(r.path)}" type="button" title="Remove from this list">${icon("x", 14)}</button>
          </div>`,
        )
        .join("")}</div>`
    : "";
}

async function openRepo(path) {
  try {
    state.overview = await invoke("open_repo", { path });
    state.page = "home";
    $("#welcome").hidden = true;
    $("#shell").hidden = false;
    render();
  } catch (e) {
    toast(String(e), { error: true });
  }
}

async function chooseRepo() {
  const path = await invoke("pick_folder");
  if (path) await openRepo(path);
}

async function refresh({ quiet = false } = {}) {
  if (!state.overview || state.busy) return;
  try {
    state.overview = await invoke("overview");
    render();
  } catch (e) {
    if (!quiet) toast(String(e), { error: true });
  }
}

// ---------------------------------------------------------------- events

document.addEventListener("click", (e) => {
  const t = e.target.closest("[data-page],[data-action],[data-forget],[data-path],[data-theme-choice]");
  if (!t) return;
  if (t.dataset.forget) {
    e.stopPropagation();
    invoke("forget_repo", { path: t.dataset.forget }).then(showWelcome);
  } else if (t.dataset.path) openRepo(t.dataset.path);
  else if (t.dataset.page) go(t.dataset.page);
  else if (t.dataset.action) ACTIONS[t.dataset.action]?.run();
  else if (t.dataset.themeChoice) applyTheme(t.dataset.themeChoice);
});

$("#open-btn").addEventListener("click", chooseRepo);
$("#repo-switch").addEventListener("click", async () => {
  await invoke("close_repo");
  showWelcome();
});
$("#refresh").addEventListener("click", () => refresh());
for (const b of document.querySelectorAll("[data-sync]")) b.addEventListener("click", () => sync(b.dataset.sync));
window.__TAURI__.event?.listen("progress", (e) => onProgress(e.payload));

document.addEventListener("keydown", (e) => {
  const mod = e.metaKey || e.ctrlKey;
  if (document.querySelector(".modal-wrap")) return;
  const typing = /^(INPUT|TEXTAREA|SELECT)$/.test(e.target.tagName);
  if (!mod && !typing && state.overview && PAGES[state.page].key?.(e)) {
    e.preventDefault();
    return;
  }
  if (mod && e.key === "o") { e.preventDefault(); chooseRepo(); }
  else if (mod && e.key === "r") { e.preventDefault(); refresh(); }
  else if (mod && /^[1-9]$/.test(e.key)) {
    const ids = NAV.flatMap((g) => g.items);
    const id = ids[Number(e.key) - 1];
    if (id && state.overview) { e.preventDefault(); go(id); }
  }
});

// Pick up changes made elsewhere (an editor, a terminal).
window.addEventListener("focus", () => refresh({ quiet: true }));
setInterval(() => { if (document.visibilityState === "visible") refresh({ quiet: true }); }, 5000);

// ---------------------------------------------------------------- start

window.addEventListener("DOMContentLoaded", start);

async function start() {
  if (/Mac/.test(navigator.platform)) document.documentElement.classList.add("mac");
  else for (const k of document.querySelectorAll(".k")) k.textContent = k.textContent.replace("⌘", "Ctrl+");
  applyTheme(savedTheme());
  $("#version").textContent = "Canopy " + (await invoke("app_version"));
  const initial = await invoke("initial_path");
  if (await invoke("smoke_mode")) return smoke(initial);
  if (initial) {
    await openRepo(initial);
    if (state.overview) return;
  }
  showWelcome();
}

// `canopy-desktop --smoke <repo>`: prove the real web view can call the Rust
// side and render Home, then quit (used by CI).
async function smoke(path) {
  try {
    state.overview = await invoke("open_repo", { path });
    $("#welcome").hidden = true;
    $("#shell").hidden = false;
    render();
    const grab = (sel) => [...document.querySelectorAll(sel)].map((e) => e.innerText.replace(/\s+/g, " ").trim());
    const home = grab(".hero h1, .tile, .step p");
    // Changes: the file list, and the first file's diff loaded from git.
    go("changes");
    for (let i = 0; i < 50 && !document.querySelector(".dl, .diff-body .diff-empty, .clean"); i++) await new Promise((r) => setTimeout(r, 100));
    const files = grab(".frow .fname");
    const lines = document.querySelectorAll(".dl").length;
    const text = [...home, `changes: ${files.join(", ") || "(clean)"}`, `diff lines: ${lines}`].join("\n");
    const ok = !!home.length && (files.length ? lines > 0 : !!document.querySelector(".clean"));
    await invoke("smoke_report", { ok, text });
  } catch (e) {
    await invoke("smoke_report", { ok: false, text: String(e) });
  }
}
