// Settings (the gear in the sidebar, or ⌘, / Ctrl+,): appearance, behavior,
// updates, the git and gh this app found, and about.
"use strict";

const setts = {
  section: "general",
  update: null, // last check_update result
  env: null, // environment()
};

async function loadSettings() {
  loadProfile();
  state.settings = await invoke("get_settings").catch(() => state.settings);
  applyTheme(state.settings.theme);
  scheduleRefresh();
  // Look for a new version at most once a day.
  const day = 24 * 3600;
  if (state.settings.check_updates && Date.now() / 1000 - (state.settings.last_update_check || 0) > day) {
    setTimeout(async () => {
      setts.update = await invoke("check_update");
      showUpdateDot();
      if (setts.update.newer) toast(`Canopy ${setts.update.latest} is out. Open Settings → Updates to get it.`);
    }, 3000);
  }
}

function showUpdateDot() {
  const dot = $("#update-dot");
  if (dot) dot.hidden = !setts.update?.newer;
}

async function saveSettings(patch) {
  try {
    state.settings = await invoke("set_settings", { settings: { ...state.settings, ...patch } });
  } catch (e) {
    toast("Couldn't save settings", { error: true, detail: String(e) });
  }
  applyTheme(state.settings.theme);
  scheduleRefresh();
}

function setTheme(choice) {
  saveSettings({ theme: choice });
}

function seg(name, value, options) {
  return `<div class="seg" role="radiogroup">${options
    .map(([v, l]) => `<button type="button" class="${String(v) === String(value) ? "on" : ""}" data-set="${name}" data-val="${esc(v)}">${esc(l)}</button>`)
    .join("")}</div>`;
}

function toggle(name, value) {
  return `<label class="switch"><input type="checkbox" data-set="${name}"${value ? " checked" : ""}><span></span></label>`;
}

function row(title, help, control) {
  return `<div class="srow2"><div><b>${title}</b>${help ? `<p>${help}</p>` : ""}</div><div class="sctl">${control}</div></div>`;
}

const INSTALL_TEXT = {
  homebrew: "Installed with Homebrew",
  winget: "Installed with winget",
  appimage: "Running as an AppImage",
  deb: "Installed from the .deb package",
  manual: "Built from source or installed by hand",
};

function updateHtml() {
  const u = setts.update;
  const s = state.settings;
  let status;
  if (!u) status = `<p class="faint">Not checked yet.</p>`;
  else if (u.error) status = `<p class="warn-text">Couldn't check: ${esc(u.error)}</p>`;
  else if (u.newer) {
    const how = u.command
      ? `<p>Run this in a terminal (it updates Canopy in place):</p>
         <div class="cmd"><code class="selectable">${esc(u.command)}</code><button class="btn small" data-s2="copy" data-text="${esc(u.command)}">Copy</button></div>`
      : `<p>Download the new version from the release page${u.install === "manual" ? ", or pull and rebuild if you run it from source" : ""}.</p>`;
    status = `<div class="update-card"><b>Canopy ${esc(u.latest)} is available.</b> You have ${esc(u.current)}.${how}
      <div class="row-btns"><button class="btn small" data-s2="open" data-url="${esc(u.url)}">What's new</button></div></div>`;
  } else status = `<p><span class="ck green">✓</span> You're up to date (Canopy ${esc(u.current)}).</p>`;
  return `<h3>Updates</h3>
    ${row("Check automatically", "Once a day, when Canopy starts. It only asks GitHub for the latest version number.", toggle("check_updates", s.check_updates))}
    <div class="srow2 col"><div class="grow-flex"><b>Version</b><p>${esc(INSTALL_TEXT[u?.install || setts.env?.install] || "")}</p></div>
      <button class="btn small" data-s2="check">Check now</button></div>
    ${status}`;
}

/// "Fix with AI": which assistant opens when CI fails.
function aiRow(s) {
  const ai = gh.ai || { installed: [] };
  const known = ["auto", "claude", "codex", "off"];
  // Settings saved before 1.0.5 have no ai_assistant: that's auto.
  const current = s.ai_assistant || "auto";
  const custom = !known.includes(current);
  const detected = ai.installed.length ? `found: ${ai.installed.join(", ")}` : "none found on this computer";
  const opts = [["auto", `Auto (${ai.installed[0] || "none found"})`], ["claude", "Claude Code"], ["codex", "Codex"], ["custom", "Custom command…"], ["off", "Off"]];
  return row("AI assistant",
    `When CI fails, "Fix with AI" writes a prompt from the error and opens this in a new terminal window (${esc(detected)}).`,
    `<select class="input small-select" id="ai-pick">${opts.map(([v, l]) => `<option value="${v}"${(custom ? "custom" : current) === v ? " selected" : ""}>${esc(l)}</option>`).join("")}</select>`)
    + (custom ? `<div class="srow2"><div><b>Command</b><p>Your assistant's command line. <code>{prompt}</code> marks where the prompt goes (otherwise it's added at the end).</p></div>
       <div class="sctl"><input class="input" id="ai-cmd" value="${esc(current === "custom" ? "" : current)}" placeholder="aider --message {prompt}" spellcheck="false"></div></div>` : "");
}

function settingsBody() {
  const s = state.settings;
  switch (setts.section) {
    case "general":
      return `<h3>General</h3>
        ${row("Theme", "", seg("theme", s.theme, [["system", "Auto"], ["light", "Light"], ["dark", "Dark"]]))}
        ${row("Show git commands", "After each action, show the git command Canopy ran, so you learn git as you go.", toggle("show_commands", s.show_commands))}
        ${row("Refresh", "How often Canopy looks for changes made outside it. It also refreshes when you switch back to the window.", seg("refresh_secs", s.refresh_secs, [[2, "2s"], [5, "5s"], [15, "15s"], [60, "1m"], [0, "Off"]]))}
        ${row("Pull", "What Pull does when your branch and the remote both have new commits. Default follows your git config (pull.rebase).", seg("pull_mode", s.pull_mode, [["default", "Default"], ["merge", "Merge"], ["rebase", "Rebase"]]))}
        ${aiRow(s)}
        ${row("Recent repositories", "The list on the welcome screen.", `<button class="btn small" data-s2="clear-recent">Clear list</button>`)}`;
    case "updates":
      return updateHtml();
    case "about":
      return `<div class="about"><img src="logo.svg" alt="" width="64" height="64"><h3>canopy ${esc(setts.env?.version || "")}</h3>
        <p>A beautiful, powerful git dashboard. MIT licensed.</p>
        <div class="row-btns">
          <button class="btn small" data-s2="open" data-url="https://shankar-sachin.github.io/canopy/">Website</button>
          <button class="btn small" data-s2="open" data-url="https://shankar-sachin.github.io/canopy/wiki/">Wiki</button>
          <button class="btn small" data-s2="open" data-url="https://github.com/shankar-sachin/canopy">Source code</button>
          <button class="btn small" data-s2="open" data-url="https://github.com/shankar-sachin/canopy/issues/new">Report a problem</button>
        </div>
        <p class="faint small">Keys: ⌘O open · ⌘R refresh · ⌘Z undo · ⌘1–9 pages · ⌘, settings (Ctrl on Linux and Windows)</p></div>`;
  }
}

function drawSettings() {
  const panel = $("#settings-panel");
  if (!panel) return;
  for (const b of panel.querySelectorAll("[data-section]")) b.classList.toggle("on", b.dataset.section === setts.section);
  panel.querySelector(".sbody").innerHTML = settingsBody();
}

async function openSettings(section) {
  if ($("#settings-panel")) return;
  if (section) setts.section = section;
  const wrap = document.createElement("div");
  wrap.className = "modal-wrap";
  const secs = [["general", "General"], ["updates", "Updates"], ["about", "About"]];
  wrap.innerHTML = `<div class="modal settings" id="settings-panel" role="dialog" aria-modal="true" aria-label="Settings">
    <nav class="snav"><h2>Settings</h2>${secs.map(([id, l]) => `<button type="button" data-section="${id}">${l}${id === "updates" ? `<span class="update-dot"${setts.update?.newer ? "" : " hidden"}></span>` : ""}</button>`).join("")}</nav>
    <div class="sbody"></div>
    <button class="btn icon ghost sclose" type="button" data-s2="close" title="Close (Esc)">${icon("x", 14)}</button>
  </div>`;
  const close = () => {
    wrap.remove();
    document.removeEventListener("keydown", onKey, true);
  };
  const onKey = (e) => {
    if (e.key === "Escape") { e.stopPropagation(); close(); }
  };
  wrap.addEventListener("click", async (e) => {
    if (e.target === wrap) return close();
    const sec = e.target.closest("[data-section]");
    if (sec) {
      setts.section = sec.dataset.section;
      return drawSettings();
    }
    const set = e.target.closest("button[data-set]");
    if (set) {
      const v = set.dataset.val;
      await saveSettings({ [set.dataset.set]: /^\d+$/.test(v) ? Number(v) : v });
      return drawSettings();
    }
    const act = e.target.closest("[data-s2]");
    if (!act) return;
    switch (act.dataset.s2) {
      case "close": return close();
      case "open": return openUrl(act.dataset.url);
      case "copy":
        try {
          await navigator.clipboard.writeText(act.dataset.text);
          act.textContent = "Copied";
        } catch {
          toast("Select the command and copy it.");
        }
        return;
      case "clear-recent":
        await invoke("clear_recent");
        toast("Cleared the recent repositories.");
        if (!state.overview) showWelcome();
        return;
      case "check":
        act.disabled = true;
        act.textContent = "Checking…";
        setts.update = await invoke("check_update");
        showUpdateDot();
        return drawSettings();
    }
  });
  wrap.addEventListener("change", async (e) => {
    const t = e.target.closest("input[data-set]");
    if (t) saveSettings({ [t.dataset.set]: t.checked });
    if (e.target.id === "ai-pick") {
      const v = e.target.value;
      await saveSettings({ ai_assistant: v === "custom" ? "custom" : v });
      gh.ai = null;
      await aiStatus();
      drawSettings();
    }
    if (e.target.id === "ai-cmd" && e.target.value.trim()) {
      await saveSettings({ ai_assistant: e.target.value.trim() });
      gh.ai = null;
      await aiStatus();
      toast("Saved the AI assistant command");
    }
  });
  document.addEventListener("keydown", onKey, true);
  document.body.append(wrap);
  drawSettings();
  if (!gh.ai) aiStatus().then(drawSettings);
  if (!setts.env) {
    setts.env = await invoke("environment").catch(() => null);
    drawSettings();
  }
}

$("#settings-btn").addEventListener("click", () => openSettings());
$("#me").addEventListener("click", () => openAccount());

// ---------------------------------------------------------------- account

function copyCmd(cmd) {
  return `<div class="cmd"><code class="selectable">${esc(cmd)}</code><button class="btn small" data-a="copy" data-text="${esc(cmd)}">Copy</button></div>`;
}

function accountHtml(a) {
  if (!a) return `<div class="acct-loading">Checking your GitHub connection…</div>`;
  const p = a;
  const connected = !!a.auth.account;
  const avatar = p.avatar_url
    ? `<img src="${esc(p.avatar_url)}${p.avatar_url.includes("?") ? "&" : "?"}s=160" alt="" width="72" height="72">`
    : `<span class="ini">${esc(initials(p.name || p.git_name || p.login)) || "?"}</span>`;
  const head = `<div class="acct-head"><span class="avatar big${p.avatar_url ? "" : " placeholder"}">${avatar}</span>
    <div><h3>${esc(p.name || p.login || p.git_name || "Not signed in")}</h3>
    ${p.login ? `<p class="mono">@${esc(p.login)}</p>` : `<p class="faint">Not signed in to GitHub</p>`}
    ${a.html_url ? `<button class="btn small" data-a="open" data-url="${esc(a.html_url)}">View profile on GitHub</button>` : ""}</div></div>`;
  const kv = (k, v) => (v ? `<div class="kv"><span>${k}</span><code class="selectable">${esc(v)}</code></div>` : "");
  const github = connected
    ? `<div class="acct-status ok"><span class="ck green">✓</span><span>Connected to <b>${esc(a.auth.host || "github.com")}</b> as <b>@${esc(a.auth.account)}</b> through the GitHub CLI</span></div>
       ${kv("Token stored in", a.auth.storage)}
       ${kv("Git protocol", a.auth.protocol)}
       ${a.auth.scopes.length ? `<div class="kv"><span>Permissions</span><span class="scopes">${a.auth.scopes.map((sc) => `<span class="scope">${esc(sc)}</span>`).join("")}</span></div>` : ""}
       <p class="faint small">Canopy never sees your token: gh talks to GitHub for it. To use another account, or sign out, run in a terminal:</p>
       ${copyCmd("gh auth login")}${copyCmd("gh auth logout")}`
    : `<div class="acct-status"><span class="ck amber">●</span><span>${a.gh ? "The GitHub CLI isn't signed in." : "The GitHub CLI isn't installed."}</span></div>
       <p class="faint small">Canopy uses the GitHub CLI with your own login for pull requests, issues and Actions. ${a.gh ? "Sign in" : "Install it, then sign in"} from a terminal:</p>
       ${a.gh ? "" : copyCmd(navigator.platform.includes("Mac") ? "brew install gh" : "https://cli.github.com")}${copyCmd("gh auth login")}`;
  const gitName = p.git_name, gitEmail = a.git_email;
  const identity = gitName || gitEmail
    ? `${kv("Name", gitName)}${kv("Email", gitEmail)}<p class="faint small">Your commits are made with this name and email.</p>`
    : `<p class="warn-text small">git doesn't know who you are yet, so it can't commit. Set your name and email:</p>
       ${copyCmd('git config --global user.name "Your Name"')}${copyCmd('git config --global user.email "you@example.com"')}`;
  return `${head}
    <h4>GitHub</h4>${github}
    <h4>Git identity</h4>${identity}
    <h4>Tools</h4>${kv("git", a.git) || `<p class="warn-text small">git wasn't found. Install git to use Canopy.</p>`}${kv("GitHub CLI", a.gh)}
    <div class="row-btns"><button class="btn small" data-a="refresh">Check again</button></div>`;
}

async function openAccount() {
  if ($("#account-panel")) return;
  const wrap = document.createElement("div");
  wrap.className = "modal-wrap";
  wrap.innerHTML = `<div class="modal account" id="account-panel" role="dialog" aria-modal="true" aria-label="Your GitHub account">
    <button class="btn icon ghost sclose" type="button" data-a="close" title="Close (Esc)">${icon("x", 14)}</button>
    <div class="abody">${accountHtml(null)}</div></div>`;
  const body = wrap.querySelector(".abody");
  const load = async () => {
    body.innerHTML = accountHtml(null);
    const a = await invoke("account").catch(() => null);
    body.innerHTML = accountHtml(a || { auth: { scopes: [] } });
  };
  const close = () => {
    wrap.remove();
    document.removeEventListener("keydown", onKey, true);
  };
  const onKey = (e) => {
    if (e.key === "Escape") { e.stopPropagation(); close(); }
  };
  wrap.addEventListener("click", async (e) => {
    if (e.target === wrap) return close();
    const act = e.target.closest("[data-a]");
    if (!act) return;
    switch (act.dataset.a) {
      case "close": return close();
      case "open": return openUrl(act.dataset.url);
      case "refresh":
        await load();
        loadProfile();
        return;
      case "copy":
        try {
          await navigator.clipboard.writeText(act.dataset.text);
          act.textContent = "Copied";
        } catch {
          toast("Select the command and copy it.");
        }
    }
  });
  document.addEventListener("keydown", onKey, true);
  document.body.append(wrap);
  load();
}

// ---------------------------------------------------------------- profile

const PERSON = '<svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true" fill="currentColor"><circle cx="8" cy="5.5" r="2.8"/><path d="M2.5 14c.6-3 2.8-4.6 5.5-4.6s4.9 1.6 5.5 4.6z"/></svg>';

function initials(name) {
  const parts = (name || "").trim().split(/\s+/).filter(Boolean);
  return (parts.length > 1 ? parts[0][0] + parts[parts.length - 1][0] : (parts[0] || "").slice(0, 2)).toUpperCase();
}

/// The avatar: your GitHub picture, else your initials, else a person icon.
async function loadProfile() {
  const p = await invoke("profile").catch(() => ({}));
  const av = $("#avatar");
  const fallback = () => {
    const ini = initials(p.name || p.git_name || p.login);
    av.innerHTML = ini ? `<span class="ini">${esc(ini)}</span>` : PERSON;
    av.classList.add("placeholder");
  };
  if (p.avatar_url) {
    const img = new Image(28, 28);
    img.alt = "";
    img.onerror = fallback;
    img.src = p.avatar_url + (p.avatar_url.includes("?") ? "&" : "?") + "s=64";
    av.replaceChildren(img);
    av.classList.remove("placeholder");
  } else fallback();
  $("#me-name").textContent = p.name || p.login || p.git_name || "Not signed in";
  $("#me-sub").textContent = p.login ? `@${p.login}` : "not on GitHub";
}
$("#welcome-settings").addEventListener("click", () => openSettings());
