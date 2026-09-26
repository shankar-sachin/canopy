// Starting a repository: shown when you open a folder that isn't one (or
// choose "Start a new one…"). Three steps: no repository here, set it up,
// connect it to GitHub (optional).
"use strict";

const su = {
  info: null, // inspect_folder()
  step: "ask", // ask | init | remote | done
  protocol: "https",
  remote: "existing", // create | existing
  busy: false,
  log: [], // commands that ran, shown at the end
};

function setupPanel() {
  return $("#setup-panel");
}

async function startSetup(path) {
  su.info = await invoke("inspect_folder", { path });
  if (su.info.is_repo) return openRepo(path);
  su.step = "ask";
  su.protocol = su.info.suggested;
  su.remote = su.info.gh_login ? "create" : "existing";
  su.log = [];
  $("#welcome").hidden = false;
  $("#shell").hidden = true;
  let wrap = $("#setup-wrap");
  if (!wrap) {
    wrap = document.createElement("div");
    wrap.id = "setup-wrap";
    wrap.className = "modal-wrap";
    wrap.innerHTML = `<div class="modal setup" id="setup-panel" role="dialog" aria-modal="true" aria-label="Start a repository"></div>`;
    document.body.append(wrap);
    wrap.addEventListener("click", onSetupClick);
    wrap.addEventListener("input", onSetupInput);
    wrap.addEventListener("change", onSetupInput);
  }
  drawSetup();
}

function closeSetup() {
  $("#setup-wrap")?.remove();
}

const STEPS = [["ask", "No repository"], ["init", "Set it up"], ["remote", "Connect"]];

function stepper() {
  const i = STEPS.findIndex(([id]) => id === su.step);
  return `<ol class="stepper">${STEPS.map(([id, l], k) => `<li class="${k < i || su.step === "done" ? "done" : k === i ? "on" : ""}"><span>${k + 1}</span>${l}</li>`).join("")}</ol>`;
}

function setupHtml() {
  const f = su.info;
  const close = `<button class="btn icon ghost sclose" type="button" data-su="cancel" title="Close (Esc)">${icon("x", 14)}</button>`;
  if (su.step === "ask") {
    return `${close}${stepper()}
      <h3>There's no git repository here yet</h3>
      <p class="mono path selectable">${esc(f.path)}</p>
      <p>A repository is a folder that git tracks: it remembers every version you commit, so you can see what changed, undo mistakes and share your work (for example on GitHub).</p>
      <p class="faint">${f.files ? `The ${plural(f.files, "file")} already in this folder stay as they are and can go into your first commit.` : "The folder is empty; Canopy can add a README to start with."}</p>
      <div class="modal-actions">
        <button class="btn ghost" data-su="other">Choose another folder</button>
        <button class="btn primary" data-su="to-init">Create a repository here</button>
      </div>`;
  }
  if (su.step === "init") {
    const opts = [["none", "No .gitignore"], ["rust", "Rust"], ["node", "Node / JavaScript"], ["python", "Python"], ["macos", "macOS only (.DS_Store)"]];
    return `${close}${stepper()}
      <h3>Set up <span class="mono">${esc(f.name)}</span></h3>
      <label class="field"><span>Branch name</span><input class="input" id="su-branch" value="${esc(f.default_branch)}" autocomplete="off" spellcheck="false"></label>
      <p class="hint">Your main line of work. Most projects call it <code>main</code>.</p>
      <label class="check field"><input type="checkbox" id="su-readme" checked> Add a README.md (what the project is, shown on GitHub)</label>
      <label class="field"><span>.gitignore (files git should leave alone, like build output)</span><select class="input" id="su-ignore">${opts.map(([v, l]) => `<option value="${v}">${esc(l)}</option>`).join("")}</select></label>
      <label class="check field"><input type="checkbox" id="su-license"> Add an MIT license (lets others use your code)</label>
      <label class="check field"><input type="checkbox" id="su-commit" checked> Make the first commit ("Initial commit")</label>
      <div class="modal-actions">
        <button class="btn ghost" data-su="back-ask">Back</button>
        <button class="btn primary" data-su="init"${su.busy ? " disabled" : ""}>${su.busy ? "Creating…" : "Create repository"}</button>
      </div>`;
  }
  if (su.step === "remote") {
    const login = f.gh_login;
    const tabs = `<div class="seg wide">${login ? `<button type="button" class="${su.remote === "create" ? "on" : ""}" data-su="tab-create">Create it on GitHub</button>` : ""}
      <button type="button" class="${su.remote === "existing" ? "on" : ""}" data-su="tab-existing">Use an existing repository</button></div>`;
    let body;
    if (su.remote === "create" && login) {
      body = `<label class="field"><span>Name on GitHub</span><input class="input" id="su-name" value="${esc(f.name)}" autocomplete="off" spellcheck="false"></label>
        <p class="hint">Creates <b class="mono" id="su-where">github.com/${esc(login)}/${esc(f.name)}</b> with your gh login, adds it as <code>origin</code>, and pushes your first commit.</p>
        <div class="choice-row">
          <label class="choice"><input type="radio" name="su-vis" value="private" checked><b>Private</b><span>Only you (and people you invite) can see it.</span></label>
          <label class="choice"><input type="radio" name="su-vis" value="public"><b>Public</b><span>Anyone can see it; only you can change it.</span></label>
        </div>
        <div class="modal-actions">
          <button class="btn ghost" data-su="skip">Skip for now</button>
          <button class="btn primary" data-su="create"${su.busy ? " disabled" : ""}>${su.busy ? "Creating on GitHub…" : "Create and push"}</button>
        </div>`;
    } else {
      const proto = (p, title, help) => `<label class="choice${su.protocol === p ? " on" : ""}"><input type="radio" name="su-proto" value="${p}"${su.protocol === p ? " checked" : ""}>
        <b>${title}${f.suggested === p ? ` <span class="pill green">suggested</span>` : ""}</b><span>${esc(help)}</span>
        ${p === "ssh" ? `<small class="${f.has_ssh_key ? "ok" : "warn-text"}">${f.has_ssh_key ? "✓ This computer has an SSH key." : "No SSH key found on this computer yet."}</small>` : ""}</label>`;
      body = `<label class="field"><span>Repository: <code>owner/name</code>, or paste its URL</span>
          <input class="input" id="su-remote" placeholder="${esc(login || "you")}/${esc(f.name)}" autocomplete="off" spellcheck="false"></label>
        <p class="hint">Make it on GitHub first (github.com/new, without a README), then paste its name here.</p>
        <div class="field"><span>How git connects to GitHub</span></div>
        <div class="choice-row">${proto("https", "HTTPS", f.https_help)}${proto("ssh", "SSH", f.ssh_help)}</div>
        <p class="hint">Address: <b class="mono selectable" id="su-url">…</b></p>
        <div class="modal-actions">
          <button class="btn ghost" data-su="skip">Skip for now</button>
          <button class="btn primary" data-su="connect"${su.busy ? " disabled" : ""}>${su.busy ? "Connecting…" : "Connect and push"}</button>
        </div>`;
    }
    return `${close}${stepper()}
      <h3>Connect it to GitHub <span class="faint">(optional)</span></h3>
      <p class="faint">Your repository lives on this computer. Connecting it to GitHub backs it up and lets you share it.</p>
      ${tabs}${body}`;
  }
  // done
  return `${close}${stepper()}
    <h3>✓ ${esc(f.name)} is ready</h3>
    <p>Here's what ran, so you can do it yourself next time:</p>
    <pre class="log selectable">${esc(su.log.join("\n"))}</pre>
    <div class="modal-actions"><button class="btn primary" data-su="open">Open it</button></div>`;
}

function drawSetup() {
  const panel = setupPanel();
  if (!panel) return;
  panel.innerHTML = setupHtml();
  if (su.step === "remote") previewUrl();
  panel.querySelector("input:not([type=checkbox]):not([type=radio])")?.focus();
}

let previewTimer;
function previewUrl() {
  clearTimeout(previewTimer);
  previewTimer = setTimeout(async () => {
    const el = $("#su-url");
    if (!el) return;
    const input = $("#su-remote")?.value || $("#su-remote")?.placeholder || "";
    const url = await invoke("remote_preview", { input, protocol: su.protocol, owner: su.info.gh_login || null });
    el.textContent = url || "type owner/name, like " + (su.info.gh_login || "you") + "/" + su.info.name;
  }, 120);
}

function onSetupInput(e) {
  const t = e.target;
  if (t.name === "su-proto") {
    su.protocol = t.value;
    for (const c of document.querySelectorAll('input[name="su-proto"]')) c.closest(".choice").classList.toggle("on", c.checked);
    previewUrl();
  } else if (t.id === "su-remote") previewUrl();
  else if (t.id === "su-name") $("#su-where").textContent = `github.com/${su.info.gh_login}/${t.value.trim() || su.info.name}`;
}

async function onSetupClick(e) {
  if (e.target.id === "setup-wrap") return;
  const b = e.target.closest("[data-su]");
  if (!b || su.busy) return;
  const f = su.info;
  switch (b.dataset.su) {
    case "cancel":
      closeSetup();
      return showWelcome();
    case "other":
      closeSetup();
      return chooseRepo();
    case "to-init":
      su.step = "init";
      return drawSetup();
    case "back-ask":
      su.step = "ask";
      return drawSetup();
    case "tab-create":
    case "tab-existing":
      su.remote = b.dataset.su.slice(4);
      return drawSetup();
    case "init": {
      const options = {
        branch: $("#su-branch").value.trim() || "main",
        readme: $("#su-readme").checked,
        gitignore: $("#su-ignore").value,
        mit_license: $("#su-license").checked,
        commit: $("#su-commit").checked,
      };
      su.busy = true;
      drawSetup();
      try {
        const res = await invoke("init_repo", { path: f.path, options });
        su.log.push(...res.cmd.split("\n"));
        su.step = "remote";
      } catch (err) {
        toast("Couldn't create the repository", { error: true, detail: String(err) });
      }
      su.busy = false;
      return drawSetup();
    }
    case "create":
    case "connect": {
      su.busy = true;
      const payload = b.dataset.su === "create"
        ? ["github_create", { name: $("#su-name").value.trim() || f.name, private: document.querySelector('input[name="su-vis"]:checked').value === "private" }]
        : null;
      let url = null;
      if (!payload) {
        const input = $("#su-remote").value.trim() || $("#su-remote").placeholder;
        url = await invoke("remote_preview", { input, protocol: su.protocol, owner: f.gh_login || null });
        if (!url) {
          su.busy = false;
          return toast("That doesn't look like a repository. Type owner/name, or paste its URL.");
        }
      }
      drawSetup();
      try {
        const res = payload ? await invoke(...payload) : await invoke("connect_remote", { url });
        su.log.push(...res.cmd.split("\n"));
        su.step = "done";
      } catch (err) {
        toast(payload ? "Couldn't create it on GitHub" : "Couldn't connect", { error: true, detail: String(err) });
      }
      su.busy = false;
      return drawSetup();
    }
    case "skip":
      su.step = "done";
      return drawSetup();
    case "open":
      closeSetup();
      return openRepo(f.path);
  }
}

document.addEventListener("keydown", (e) => {
  if (e.key === "Escape" && $("#setup-wrap") && !su.busy) {
    closeSetup();
    showWelcome();
  }
});

$("#new-btn").addEventListener("click", async () => {
  const path = await invoke("pick_folder");
  if (path) startSetup(path);
});
