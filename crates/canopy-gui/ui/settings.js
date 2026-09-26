// Settings (the gear in the sidebar, or ⌘, / Ctrl+,): appearance, behavior,
// updates, the git and gh this app found, and about.
"use strict";

const setts = {
  section: "general",
  update: null, // last check_update result
  env: null, // environment()
};

async function loadSettings() {
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

function envHtml() {
  const e = setts.env;
  if (!e) return `<h3>Git &amp; GitHub</h3><p class="faint">Looking…</p>`;
  const line = (label, value, missing) =>
    `<div class="kv"><span>${label}</span>${value ? `<code class="selectable">${esc(value)}</code>` : `<span class="warn-text">${missing}</span>`}</div>`;
  return `<h3>Git &amp; GitHub</h3>
    <p class="faint">Canopy runs your own git and GitHub CLI, so your config, hooks, signing and logins all apply.</p>
    ${line("git", e.git, "not found: install git")}
    ${line("GitHub CLI", e.gh, "not found (only needed for the GitHub pages)")}
    ${e.gh ? line("GitHub account", e.gh_user, "not logged in: run gh auth login") : ""}
    ${line("Settings file", e.config_file, "none")}`;
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
        ${row("Recent repositories", "The list on the welcome screen.", `<button class="btn small" data-s2="clear-recent">Clear list</button>`)}`;
    case "git":
      return envHtml();
    case "updates":
      return updateHtml();
    case "about":
      return `<div class="about"><img src="logo.svg" alt="" width="64" height="64"><h3>Canopy ${esc(setts.env?.version || "")}</h3>
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
  const secs = [["general", "General"], ["git", "Git & GitHub"], ["updates", "Updates"], ["about", "About"]];
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
  wrap.addEventListener("change", (e) => {
    const t = e.target.closest("input[data-set]");
    if (t) saveSettings({ [t.dataset.set]: t.checked });
  });
  document.addEventListener("keydown", onKey, true);
  document.body.append(wrap);
  drawSettings();
  if (!setts.env) {
    setts.env = await invoke("environment").catch(() => null);
    drawSettings();
  }
}

$("#settings-btn").addEventListener("click", () => openSettings());
$("#welcome-settings").addEventListener("click", () => openSettings());
