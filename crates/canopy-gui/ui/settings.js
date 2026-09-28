// Settings (the gear in the sidebar, or ⌘, / Ctrl+,): appearance, behavior,
// updates, the git and gh this app found, and about.
"use strict";

const setts = {
  section: "general",
  update: null, // last check_update result
  env: null, // environment()
  editors: null, // detect_editors()
  editing: null, // the theme being customized: { name, base, colors, existing }
};

async function loadSettings() {
  loadProfile();
  state.settings = await invoke("get_settings").catch(() => state.settings);
  await loadThemes();
  applyTheme(state.settings.theme);
  scheduleRefresh();
  // Look for a new version at most once a day.
  const day = 24 * 3600;
  if (state.settings.check_updates && Date.now() / 1000 - (state.settings.last_update_check || 0) > day) {
    setTimeout(async () => {
      setts.update = await invoke("check_update").catch((e) => ({ error: String(e) }));
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

async function loadThemes() {
  state.themes = await invoke("list_themes").catch(() => state.themes || null);
}

// Coming back from editing a theme's JSON: pick up the change.
window.addEventListener("focus", async () => {
  if (setts.editing) return;
  await loadThemes();
  applyTheme(state.settings.theme);
  if ($("#settings-panel") && setts.section === "appearance") drawSettings();
});

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

// ---------------------------------------------------------------- themes

const NATIVE_LABELS = { system: "Auto (follows your computer)", dark: "Canopy", light: "Canopy Light" };

function themeLabel(name) {
  return NATIVE_LABELS[name] || name;
}

function themeByName(name) {
  return state.themes?.themes.find((t) => t.name === name);
}

/// The palette behind a choice ("dark" is the terminal app's canopy theme).
function paletteFor(name) {
  if (name === "system") name = matchMedia("(prefers-color-scheme: light)").matches ? "light" : "dark";
  return themeByName(name === "dark" ? "canopy" : name);
}

function swatchStrip(c) {
  if (!c) return "";
  return `<span class="tstrip" style="background:${c.bg};border-color:${c.border}">
    ${["accent", "added", "removed", "modified", "hash", "branch"].map((k) => `<i style="background:${c[k]}"></i>`).join("")}
    <b style="color:${c.fg}">Aa</b></span>`;
}

function themeCard(id, label, colors, extra = "") {
  const on = state.settings.theme === id;
  const auto = id === "system" ? ` auto` : "";
  return `<button type="button" class="tcard${on ? " on" : ""}${auto}" data-theme-pick="${esc(id)}" aria-pressed="${on}">
    ${id === "system" ? `<span class="tsplit">${swatchStrip(themeByName("canopy")?.colors)}${swatchStrip(themeByName("light")?.colors)}</span>` : swatchStrip(colors)}
    <span class="tname">${esc(label)}${extra}</span></button>`;
}

function appearanceHtml() {
  const ts = state.themes;
  if (!ts) return `<h3>Appearance</h3><p class="faint">Loading themes…</p>`;
  const builtin = ts.themes.filter((t) => t.builtin && t.name !== "canopy" && t.name !== "light");
  const mine = ts.themes.filter((t) => !t.builtin);
  const cur = themeByName(state.settings.theme);
  const custom = cur && !cur.builtin;
  return `<h3>Appearance</h3>
    <div class="tgrid">
      ${themeCard("system", "Auto")}
      ${themeCard("dark", "Canopy", themeByName("canopy")?.colors)}
      ${themeCard("light", "Canopy Light", themeByName("light")?.colors)}
      ${builtin.map((t) => themeCard(t.name, t.name, t.colors)).join("")}
    </div>
    <h3 class="sub">Your themes</h3>
    ${mine.length ? `<div class="tgrid">${mine.map((t) => themeCard(t.name, t.name, t.colors)).join("")}</div>`
      : `<p class="faint small">None yet. Customize any theme to make one; it's saved where the terminal app finds it too.</p>`}
    <div class="row-btns tactions">
      <button class="btn small primary" data-s2="customize">${custom ? "Edit" : "Customize"} ${esc(themeLabel(state.settings.theme))}…</button>
      ${custom ? `<button class="btn small" data-s2="theme-json">Edit JSON</button>` : ""}
      <button class="btn small" data-s2="theme-export">Export…</button>
      <button class="btn small" data-s2="theme-import">Import…</button>
      ${custom ? `<button class="btn small ghost" data-s2="theme-delete">Delete</button>` : ""}
    </div>
    <p class="faint small">Themes are JSON files in <code class="selectable">${esc(ts.folder)}</code>. The terminal app uses the same ones: <code>canopy -t "name"</code>, or <code>theme = "name"</code> in its config. <a href="#" data-s2="open" data-url="https://shankar-sachin.github.io/canopy/wiki/themes.html#your-own">How the file works</a></p>`;
}

const FIELD_GROUPS = [
  ["Base", ["bg", "fg", "muted", "border", "border_focus", "selection_bg"]],
  ["Highlights", ["accent", "accent_alt", "warn", "error"]],
  ["Changes", ["added", "removed", "modified", "conflict", "added_bg", "removed_bg"]],
  ["Git", ["hash", "branch", "remote", "tag"]],
];

/// A little terminal-app screen in the theme's colors, so every color shows.
function tuiPreview(c) {
  const sp = (k, t, bold) => `<span style="color:${c[k]}${bold ? ";font-weight:700" : ""}">${t}</span>`;
  return `<div class="tui-prev" style="background:${c.bg};color:${c.fg};border-color:${c.border_focus}">
    <div>${sp("accent", "canopy", true)} ${sp("muted", "·")} ${sp("branch", "● main")} ${sp("remote", "↑1 origin/main")} ${sp("tag", "v1.0.7")}</div>
    <div class="tp-box" style="border-color:${c.border}"><div style="background:${c.selection_bg}">${sp("hash", "f5be172")} Add a theme editor ${sp("muted", "2h ago")}</div>
      <div>${sp("hash", "37e26d5")} Details for failed runs ${sp("muted", "1d ago")}</div></div>
    <div>${sp("modified", "M")} src/app.rs  ${sp("added", "A")} themes.rs  ${sp("conflict", "U")} config.toml</div>
    <div style="background:${c.added_bg}">${sp("added", "+ let theme = Theme::find(name);")}</div>
    <div style="background:${c.removed_bg}">${sp("removed", "- let theme = Theme::by_name(name);")}</div>
    <div>${sp("accent_alt", "Tip:")} ${sp("warn", "2 behind")} ${sp("error", "✕ CI failed")}</div></div>`;
}

function themeEditorHtml() {
  const e = setts.editing;
  const f = state.themes.fields.reduce((m, [k, d]) => ((m[k] = d), m), {});
  const inputs = FIELD_GROUPS.map(([title, keys]) => `<div class="tgroup"><b>${title}</b>${keys.map((k) => `
      <label class="tcolor" title="${esc(f[k] || k)}"><input type="color" data-color="${k}" value="${e.colors[k]}"><span>${esc(f[k] || k)}</span><code>${k}</code>
        <input class="input hex" data-hex="${k}" value="${e.colors[k]}" maxlength="7" spellcheck="false"></label>`).join("")}</div>`).join("");
  return `<h3>${e.existing ? "Edit" : "New"} theme</h3>
    <div class="tedit">
      <label class="field"><span>Name</span><input class="input" id="t-name" value="${esc(e.name)}" autocomplete="off" spellcheck="false"></label>
      <p class="hint">Starts from <b>${esc(themeLabel(e.base === "canopy" ? "dark" : e.base))}</b>; the app changes as you pick colors. Some colors only show in the terminal app, so here's a preview of it too:</p>
      <div id="t-prev">${tuiPreview(e.colors)}</div>
      <div class="tgroups">${inputs}</div>
    </div>
    <div class="modal-actions sticky">
      <button class="btn ghost" data-s2="theme-cancel">Cancel</button>
      <button class="btn primary" data-s2="theme-save">Save theme</button>
    </div>`;
}

function startEditing() {
  const name = state.settings.theme;
  const cur = paletteFor(name);
  const existing = cur && !cur.builtin;
  setts.editing = {
    name: existing ? cur.name : `My ${cur?.name || "theme"}`,
    base: existing ? cur.base : cur?.name || "canopy",
    colors: { ...(cur?.colors || {}) },
    existing,
  };
}

function previewEditing() {
  setThemeVars(paletteVars(setts.editing.colors));
  const p = $("#t-prev");
  if (p) p.innerHTML = tuiPreview(setts.editing.colors);
}

function stopEditing() {
  if (!setts.editing) return;
  setts.editing = null;
  applyTheme(state.settings.theme);
}

async function saveEditing() {
  const e = setts.editing;
  e.name = ($("#t-name").value || "").trim();
  if (!e.name) return toast("Give the theme a name.");
  // Only what differs from the base goes in the file, so it stays short.
  const base = themeByName(e.base)?.colors || {};
  const colors = Object.fromEntries(Object.entries(e.colors).filter(([k, v]) => v.toLowerCase() !== (base[k] || "").toLowerCase()));
  try {
    const path = await invoke("save_theme", { theme: { name: e.name, base: e.base, colors } });
    setts.editing = null;
    await loadThemes();
    await saveSettings({ theme: e.name });
    toast(`Saved "${e.name}". The terminal app can use it too: canopy -t "${e.name}"`, { detail: path });
  } catch (err) {
    toast("Couldn't save the theme", { error: true, detail: String(err) });
  }
}

// ---------------------------------------------------------------- editor

function editorRow(s) {
  const found = setts.editors || [];
  const ids = ["system", ...found.map((e) => e.id)];
  const custom = !ids.includes(s.editor);
  const opts = [["system", "System default"], ...found.map((e) => [e.id, e.name + (e.terminal ? " (in a terminal)" : "")]), ["custom", "Custom command…"]];
  const help = setts.editors
    ? `"Open in editor" on Changes and Details uses this. ${found.length ? `Found ${found.map((e) => e.name).join(", ")}.` : "No editors found on your PATH."}`
    : `"Open in editor" on Changes and Details uses this.`;
  return row("Text editor", esc(help),
    `<select class="input small-select" id="editor-pick">${opts.map(([v, l]) => `<option value="${v}"${(custom ? "custom" : s.editor) === v ? " selected" : ""}>${esc(l)}</option>`).join("")}</select>`)
    + (custom ? `<div class="srow2"><div><b>Command</b><p>Your editor's command line; the file is added at the end.</p></div>
       <div class="sctl"><input class="input" id="editor-cmd" value="${esc(s.editor === "custom" ? "" : s.editor)}" placeholder="idea --line 1" spellcheck="false"></div></div>` : "");
}

// A pointer to the terminal app, for when you want more power.
function terminalRow() {
  const env = setts.env;
  if (!env) return "";
  if (env.terminal_app) {
    return row("Canopy for the terminal", `Installed (${esc(env.terminal_app)}). Run <code>canopy</code> in a repository for everything, from the keyboard: line staging, interactive rebase, bisect, worktrees and your own commands.`, "");
  }
  return `<div class="srow2 stack"><div><b>Canopy for the terminal</b>
      <p>Want more power? The terminal app does everything this one does and more, from the keyboard: interactive rebase, bisect, worktrees, every repository at once, and your own commands. New power features land there first.</p></div>
    <div class="cmd"><code class="selectable">${esc(env.terminal_install)}</code><button class="btn small" data-s2="copy" data-text="${esc(env.terminal_install)}">Copy</button></div></div>`;
}

function settingsBody() {
  const s = state.settings;
  switch (setts.section) {
    case "general":
      return `<h3>General</h3>
        ${row("Theme", `${esc(themeLabel(s.theme))}. Pick another or make your own in Appearance.`, `<button class="btn small" data-section="appearance">Appearance…</button>`)}
        ${editorRow(s)}
        ${row("Show git commands", "After each action, show the git command Canopy ran, so you learn git as you go.", toggle("show_commands", s.show_commands))}
        ${row("Refresh", "How often Canopy looks for changes made outside it. It also refreshes when you switch back to the window.", seg("refresh_secs", s.refresh_secs, [[2, "2s"], [5, "5s"], [15, "15s"], [60, "1m"], [0, "Off"]]))}
        ${row("Pull", "What Pull does when your branch and the remote both have new commits. Default follows your git config (pull.rebase).", seg("pull_mode", s.pull_mode, [["default", "Default"], ["merge", "Merge"], ["rebase", "Rebase"]]))}
        ${aiRow(s)}
        ${row("Recent repositories", "The list on the welcome screen.", `<button class="btn small" data-s2="clear-recent">Clear list</button>`)}
        ${row("New to git?", "The short introduction on Home: changes, staging, commits and pushing.", `<button class="btn small" data-s2="show-intro"${s.seen_intro ? "" : " disabled"}>${s.seen_intro ? "Show it again" : "Showing on Home"}</button>`)}
        <h3 class="sub">Terminal app</h3>
        ${terminalRow()}
        ${row("Suggest the terminal app", "Now and then, mention Canopy for the terminal (only if it isn't installed).", toggle("terminal_tip", s.terminal_tip))}
        ${row("Share settings", "The theme, text editor, AI assistant and git-command setting can move between this app and the terminal app (<code>canopy</code>), which keeps them in <code>config.toml</code>. Only those settings change.",
          `<div class="row-btns"><button class="btn small" data-s2="from-tui">Import from terminal app</button><button class="btn small" data-s2="to-tui">Send to terminal app</button></div>`)}
        ${row("Settings file", "Save them as a JSON file (for another computer, or <code>canopy --import-settings</code>), or load one.",
          `<div class="row-btns"><button class="btn small" data-s2="export-settings">Export…</button><button class="btn small" data-s2="import-settings">Import…</button></div>`)}`;
    case "appearance":
      return setts.editing ? themeEditorHtml() : appearanceHtml();
    case "updates":
      return updateHtml();
    case "about":
      return `<div class="about"><img src="logo.svg" alt="" width="64" height="64"><h3>canopy ${esc(setts.env?.version || "")}</h3>
        <p>Git, at a glance: a friendly desktop app for git and GitHub. MIT licensed.</p>
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
  const secs = [["general", "General"], ["appearance", "Appearance"], ["updates", "Updates"], ["about", "About"]];
  wrap.innerHTML = `<div class="modal settings" id="settings-panel" role="dialog" aria-modal="true" aria-label="Settings">
    <nav class="snav"><h2>Settings</h2>${secs.map(([id, l]) => `<button type="button" data-section="${id}">${l}${id === "updates" ? `<span class="update-dot"${setts.update?.newer ? "" : " hidden"}></span>` : ""}</button>`).join("")}</nav>
    <div class="sbody"></div>
    <button class="btn icon ghost sclose" type="button" data-s2="close" title="Close (Esc)">${icon("x", 14)}</button>
  </div>`;
  const close = () => {
    stopEditing();
    wrap.remove();
    document.removeEventListener("keydown", onKey, true);
  };
  const onKey = (e) => {
    if (e.key === "Escape") { e.stopPropagation(); close(); }
  };
  wrap.addEventListener("click", async (e) => {
    if (e.target === wrap) return close();
    const pick = e.target.closest("[data-theme-pick]");
    if (pick) {
      await saveSettings({ theme: pick.dataset.themePick });
      return drawSettings();
    }
    const sec = e.target.closest("[data-section]");
    if (sec) {
      stopEditing();
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
    e.preventDefault();
    const shared = async (p) => {
      state.settings = p;
      await loadThemes();
      applyTheme(p.theme);
      gh.ai = null;
      aiStatus().then(drawSettings);
    };
    switch (act.dataset.s2) {
      case "close": return close();
      case "customize":
        startEditing();
        return drawSettings();
      case "theme-cancel":
        stopEditing();
        return drawSettings();
      case "theme-save":
        await saveEditing();
        return drawSettings();
      case "theme-json": {
        const t = themeByName(state.settings.theme);
        return invoke("open_in_editor", { path: t.path })
          .then((w) => toast(`Opened it in ${w}. Save the file and come back; Canopy picks up the change.`))
          .catch((err) => toast("Couldn't open the editor", { error: true, detail: String(err) }));
      }
      case "theme-export": {
        const name = paletteFor(state.settings.theme)?.name || "canopy";
        const path = await invoke("export_theme", { name }).catch((err) => toast("Couldn't export", { error: true, detail: String(err) }));
        if (path) toast(`Exported ${name}`, { detail: path });
        return;
      }
      case "theme-import": {
        try {
          const name = await invoke("import_theme");
          if (!name) return;
          await loadThemes();
          await saveSettings({ theme: name });
          toast(`Imported "${name}"`);
        } catch (err) {
          toast("Couldn't import that theme", { error: true, detail: String(err) });
        }
        return drawSettings();
      }
      case "theme-delete": {
        const name = state.settings.theme;
        const ok = await ask({ title: `Delete the theme "${name}"?`, text: "Its file is removed, so the terminal app loses it too.", buttons: [{ label: "Delete", value: true, kind: "danger" }] });
        if (!ok) return;
        await invoke("delete_theme", { name }).catch((err) => toast("Couldn't delete it", { error: true, detail: String(err) }));
        await loadThemes();
        await saveSettings({ theme: "system" });
        return drawSettings();
      }
      case "from-tui":
        try {
          await shared(await invoke("import_from_terminal"));
          toast("Took the terminal app's theme, editor and AI settings");
        } catch (err) {
          toast("Couldn't read the terminal app's settings", { error: true, detail: String(err) });
        }
        return drawSettings();
      case "to-tui":
        try {
          const path = await invoke("send_to_terminal");
          toast("The terminal app now uses these settings", { detail: path });
        } catch (err) {
          toast("Couldn't update the terminal app's config", { error: true, detail: String(err) });
        }
        return;
      case "export-settings": {
        const path = await invoke("export_settings").catch((err) => toast("Couldn't export", { error: true, detail: String(err) }));
        if (path) toast("Exported your settings", { detail: path });
        return;
      }
      case "import-settings":
        try {
          const p = await invoke("import_settings");
          if (!p) return;
          await shared(p);
          toast("Imported the settings");
        } catch (err) {
          toast("Couldn't import those settings", { error: true, detail: String(err) });
        }
        return drawSettings();
      case "open": return openUrl(act.dataset.url);
      case "copy":
        try {
          await navigator.clipboard.writeText(act.dataset.text);
          act.textContent = "Copied";
        } catch {
          toast("Select the command and copy it.");
        }
        return;
      case "show-intro":
        await saveSettings({ seen_intro: false });
        toast("The introduction is back on Home.");
        if (state.overview) render();
        return drawSettings();
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
  wrap.addEventListener("input", (e) => {
    const k = e.target.dataset.color || e.target.dataset.hex;
    if (!k || !setts.editing) return;
    let v = e.target.value.trim().toLowerCase();
    if (e.target.dataset.hex) {
      if (!v.startsWith("#")) v = "#" + v;
      if (/^#[0-9a-f]{3}$/.test(v)) v = "#" + [...v.slice(1)].map((x) => x + x).join("");
      if (!/^#[0-9a-f]{6}$/.test(v)) return;
      $(`[data-color="${k}"]`).value = v;
    } else $(`[data-hex="${k}"]`).value = v;
    setts.editing.colors[k] = v;
    previewEditing();
  });
  wrap.addEventListener("change", async (e) => {
    if (e.target.id === "editor-pick") {
      await saveSettings({ editor: e.target.value });
      return drawSettings();
    }
    if (e.target.id === "editor-cmd" && e.target.value.trim()) {
      await saveSettings({ editor: e.target.value.trim() });
      return toast("Saved the editor command");
    }
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
  if (!setts.editors) invoke("detect_editors").then((e) => { setts.editors = e; drawSettings(); }).catch(() => {});
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
