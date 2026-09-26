// Changes: the working tree. Stage by file, hunk or line; discard; resolve
// conflicts; commit.
"use strict";

const ch = {
  sel: null, // { path, section: "conflicts" | "staged" | "unstaged" }
  diff: null, // [FileDiff] for sel
  diffKey: "", // what `diff` was loaded for, to avoid redrawing on every refresh
  lines: new Set(), // selected "hunk:line" keys
  anchor: null, // last clicked line key, for shift-click ranges
  loading: 0,
};

const CODE = { Modified: "M", Added: "A", Deleted: "D", Renamed: "R", Copied: "C", TypeChanged: "T", Unmerged: "U" };

function sections(o) {
  const files = o.status.files;
  const conflicts = files.filter((f) => f.kind === "Conflicted");
  const staged = files.filter((f) => f.kind === "Tracked" && f.index !== "Unmodified");
  const unstaged = files.filter((f) => f.kind === "Untracked" || (f.kind === "Tracked" && f.worktree !== "Unmodified"));
  return { conflicts, staged, unstaged };
}

function fileBadge(f, section) {
  if (section === "conflicts") return `<span class="fcode red">!</span>`;
  if (f.kind === "Untracked") return `<span class="fcode green" title="New file">+</span>`;
  const c = section === "staged" ? f.index : f.worktree;
  const cls = c === "Deleted" ? "red" : c === "Added" ? "green" : "amber";
  return `<span class="fcode ${cls}" title="${c}">${CODE[c] || "?"}</span>`;
}

function splitPath(p) {
  const i = p.lastIndexOf("/");
  return i < 0 ? ["", p] : [p.slice(0, i + 1), p.slice(i + 1)];
}

function fileRow(f, section) {
  const [dir, base] = splitPath(f.path);
  const on = ch.sel && ch.sel.path === f.path && ch.sel.section === section;
  const btn = section === "staged"
    ? `<button class="btn small ghost" data-ch="unstage" title="Unstage">−</button>`
    : section === "unstaged"
      ? `<button class="btn small ghost" data-ch="stage" title="Stage">+</button>`
      : "";
  const renamed = f.orig_path ? ` <span class="faint">← ${esc(f.orig_path)}</span>` : "";
  return `<li class="frow${on ? " on" : ""}" data-path="${esc(f.path)}" data-section="${section}" data-untracked="${f.kind === "Untracked"}">
    ${fileBadge(f, section)}<span class="fname"><b>${esc(base)}</b> <span class="faint">${esc(dir)}</span>${renamed}</span>${btn}
  </li>`;
}

function listHtml(o) {
  const s = sections(o);
  const group = (key, title, files, action) =>
    files.length
      ? `<div class="fgroup"><div class="fgroup-h"><span>${title}</span><span class="badge">${files.length}</span>${action}</div>
         <ul class="flist">${files.map((f) => fileRow(f, key)).join("")}</ul></div>`
      : "";
  const html =
    group("conflicts", "Conflicts", s.conflicts, "") +
    group("staged", "Staged", s.staged, `<button class="btn small ghost" data-ch="unstage-all">Unstage all</button>`) +
    group("unstaged", "Changes", s.unstaged, `<button class="btn small ghost" data-ch="stage-all">Stage all</button>`);
  return html || `<div class="clean"><div class="clean-mark">✓</div><b>Working tree clean</b><p>Nothing to commit. Edit some files and they'll show up here.</p></div>`;
}

const STATE_VERB = { Merging: "merge", Rebasing: "rebase", CherryPicking: "cherry-pick", Reverting: "revert" };

function opBanner(o) {
  const verb = STATE_VERB[o.state];
  if (!verb) return "";
  const left = o.counts.conflicts;
  return `<div class="op-banner"><b>A ${verb} is in progress.</b>
    <span>${left ? `Resolve ${plural(left, "conflict")}, then continue.` : "No conflicts left: continue when you're ready."}</span>
    <button class="btn small" data-ch="abort">Abort</button>
    <button class="btn small primary" data-ch="continue"${left ? " disabled" : ""}>Continue ${verb}</button></div>`;
}

function commitBox(o) {
  return `<div class="commit-box">
    <input id="c-summary" class="input" placeholder="Summary (required)" maxlength="200" autocomplete="off" spellcheck="true">
    <textarea id="c-body" class="input" placeholder="Description" rows="3" spellcheck="true"></textarea>
    <div class="commit-row">
      <label class="check"><input type="checkbox" id="c-amend"> Amend last commit</label>
      <button class="btn primary" id="c-go" type="button"></button>
    </div>
  </div>`;
}

function updateCommitButton(o) {
  const btn = $("#c-go");
  if (!btn) return;
  const amend = $("#c-amend").checked;
  const n = o.counts.staged;
  const k = navigator.platform.includes("Mac") ? "⌘↵" : "Ctrl+↵";
  if (amend) btn.innerHTML = `Amend <kbd class="k">${k}</kbd>`;
  else if (n) btn.innerHTML = `Commit ${plural(n, "file")} <kbd class="k">${k}</kbd>`;
  else if (o.counts.unstaged) btn.innerHTML = `Stage all &amp; commit <kbd class="k">${k}</kbd>`;
  else btn.textContent = "Nothing to commit";
  btn.disabled = !amend && !n && !o.counts.unstaged;
}

// ---------------------------------------------------------------- diff

function diffHtml() {
  if (!ch.sel) return `<div class="diff-empty">Select a file to see its changes.</div>`;
  const { path, section } = ch.sel;
  if (section === "conflicts") {
    return `<div class="diff-head"><b class="mono">${esc(path)}</b><span class="pill red">conflict</span></div>
      <div class="conflict-help">
        <p>Both sides changed this file. Open it in your editor and pick what to keep between the
        <code>&lt;&lt;&lt;&lt;&lt;&lt;&lt;</code> and <code>&gt;&gt;&gt;&gt;&gt;&gt;&gt;</code> markers, then mark it resolved.
        Or take one side for the whole file:</p>
        <div class="conflict-actions">
          <button class="btn" data-ch="ours">Keep mine (ours)</button>
          <button class="btn" data-ch="theirs">Take theirs</button>
          <button class="btn primary" data-ch="resolved">Mark resolved</button>
        </div>
      </div>`;
  }
  if (!ch.diff) return `<div class="diff-empty">Loading…</div>`;
  const unstage = section === "staged";
  const fileBtns = unstage
    ? `<button class="btn small" data-ch="unstage">Unstage file</button>`
    : `<button class="btn small" data-ch="discard">Discard…</button><button class="btn small primary" data-ch="stage">Stage file</button>`;
  let body = "";
  for (const file of ch.diff) {
    if (file.binary) body += `<div class="diff-empty">Binary file: no text diff.</div>`;
    file.hunks.forEach((h, hi) => {
      body += `<div class="hunk"><div class="hunk-h"><span class="mono">${esc(h.header)}</span>
        <button class="btn small ghost" data-ch="hunk" data-hunk="${hi}">${unstage ? "Unstage" : "Stage"} hunk</button></div>`;
      h.lines.forEach((l, li) => {
        const key = `${hi}:${li}`;
        const change = l.kind === "Added" || l.kind === "Removed";
        const cls = { Added: "add", Removed: "del", Context: "ctx", NoNewline: "nonl" }[l.kind];
        const sign = { Added: "+", Removed: "−", Context: " ", NoNewline: "" }[l.kind];
        body += `<div class="dl ${cls}${ch.lines.has(key) ? " sel" : ""}${change ? " pick" : ""}" data-line="${key}">
          <span class="no">${l.old_no ?? ""}</span><span class="no">${l.new_no ?? ""}</span><span class="sign">${sign}</span><span class="code">${esc(l.content) || " "}</span></div>`;
      });
      body += `</div>`;
    });
    if (!file.binary && !file.hunks.length) body += `<div class="diff-empty">No text changes (maybe only the file mode changed).</div>`;
  }
  const n = ch.lines.size;
  const lineBar = n
    ? `<div class="line-bar"><span>${plural(n, "line")} selected</span><button class="btn small ghost" data-ch="clear-lines">Clear</button>
       <button class="btn small primary" data-ch="lines">${unstage ? "Unstage" : "Stage"} ${plural(n, "line")}</button></div>`
    : "";
  return `<div class="diff-head"><b class="mono">${esc(path)}</b><span class="pill">${unstage ? "staged" : ch.sel.untracked ? "new file" : "unstaged"}</span>
      <span class="grow"></span>${fileBtns}</div>
    ${lineBar}
    <div class="diff-body selectable-code">${body || `<div class="diff-empty">No changes.</div>`}</div>
    <div class="diff-tip">Click a <b>+</b> or <b>−</b> line to pick it (shift-click for a range), then stage just those lines.</div>`;
}

async function loadDiff() {
  const sel = ch.sel;
  if (!sel || sel.section === "conflicts") {
    ch.diff = null;
    drawDiff();
    return;
  }
  const token = ++ch.loading;
  try {
    const diff = await invoke("file_diff", { path: sel.path, staged: sel.section === "staged", untracked: !!sel.untracked });
    if (token !== ch.loading) return;
    const key = JSON.stringify(diff);
    if (key !== ch.diffKey) {
      ch.diff = diff;
      ch.diffKey = key;
      ch.lines.clear();
      ch.anchor = null;
      drawDiff();
    }
  } catch (e) {
    if (token === ch.loading) {
      ch.diff = [];
      drawDiff();
      toast(String(e), { error: true });
    }
  }
}

function drawDiff() {
  const pane = $("#diff-pane");
  if (!pane) return;
  const body = pane.querySelector(".diff-body");
  const top = body ? body.scrollTop : 0;
  pane.innerHTML = diffHtml();
  const nb = pane.querySelector(".diff-body");
  if (nb) nb.scrollTop = top;
}

/// Keep the selection on the same file if it's still listed; otherwise pick
/// the first file. Returns true when the selection moved.
function fixSelection(o) {
  const s = sections(o);
  const all = [...s.conflicts.map((f) => [f, "conflicts"]), ...s.unstaged.map((f) => [f, "unstaged"]), ...s.staged.map((f) => [f, "staged"])];
  const find = (path, section) => all.find(([f, sec]) => f.path === path && (!section || sec === section));
  const hit = ch.sel && (find(ch.sel.path, ch.sel.section) || find(ch.sel.path));
  const next = hit || all[0];
  const sel = next ? { path: next[0].path, section: next[1], untracked: next[0].kind === "Untracked" } : null;
  const moved = JSON.stringify(sel) !== JSON.stringify(ch.sel);
  ch.sel = sel;
  if (moved) {
    ch.diff = null;
    ch.diffKey = "";
    ch.lines.clear();
  }
  return moved;
}

// ---------------------------------------------------------------- page

function select(path, section, untracked) {
  ch.sel = { path, section, untracked };
  ch.diff = null;
  ch.diffKey = "";
  ch.lines.clear();
  for (const r of document.querySelectorAll(".frow")) r.classList.toggle("on", r.dataset.path === path && r.dataset.section === section);
  drawDiff();
  loadDiff();
}

async function doCommit() {
  const o = state.overview;
  const summary = $("#c-summary").value.trim();
  const body = $("#c-body").value.trim();
  const amend = $("#c-amend").checked;
  if (!summary) {
    $("#c-summary").focus();
    toast("Write a short summary of what changed first.");
    return;
  }
  if (!amend && !o.counts.staged) {
    if (!o.counts.unstaged) return;
    if (!(await run("Stage all", "stage_all"))) return;
  }
  const message = body ? `${summary}\n\n${body}` : summary;
  const res = await run(amend ? "Amended the last commit" : "Committed", "commit", { message, amend });
  if (res) {
    $("#c-summary").value = "";
    $("#c-body").value = "";
    $("#c-amend").checked = false;
    updateCommitButton(state.overview);
  }
}

async function onAction(what, el) {
  const o = state.overview;
  const row = el.closest(".frow");
  const target = row ? { path: row.dataset.path, section: row.dataset.section, untracked: row.dataset.untracked === "true" } : ch.sel;
  switch (what) {
    case "stage": return run(`Staged ${target.path}`, "stage", { paths: [target.path] });
    case "unstage": return run(`Unstaged ${target.path}`, "unstage", { paths: [target.path] });
    case "stage-all": return run("Staged everything", "stage_all");
    case "unstage-all": return run("Unstaged everything", "unstage_all");
    case "discard": {
      const ok = await ask({
        title: "Discard changes?",
        text: target.untracked
          ? `This deletes the new file ${target.path}. It can't be undone.`
          : `This throws away your unstaged edits to ${target.path}. Staged changes stay. It can't be undone.`,
        buttons: [{ label: target.untracked ? "Delete file" : "Discard", value: true, kind: "danger" }],
      });
      if (!ok) return;
      return run(`Discarded ${target.path}`, "discard", target.untracked ? { tracked: [], untracked: [target.path] } : { tracked: [target.path], untracked: [] });
    }
    case "hunk": {
      const file = ch.diff[0];
      const hunk = file.hunks[Number(el.dataset.hunk)];
      const unstage = ch.sel.section === "staged";
      const lines = hunk.lines.map((_, i) => i);
      return run(unstage ? "Unstaged hunk" : "Staged hunk", "apply_lines", { file, hunk, lines, unstage });
    }
    case "lines": {
      const file = ch.diff[0];
      const unstage = ch.sel.section === "staged";
      // One patch per hunk, bottom-most first so earlier line numbers stay valid.
      const byHunk = new Map();
      for (const k of ch.lines) {
        const [hi, li] = k.split(":").map(Number);
        if (!byHunk.has(hi)) byHunk.set(hi, []);
        byHunk.get(hi).push(li);
      }
      const n = ch.lines.size;
      for (const hi of [...byHunk.keys()].sort((a, b) => b - a)) {
        const res = await invoke("apply_lines", { file, hunk: file.hunks[hi], lines: byHunk.get(hi), unstage }).catch((e) => {
          toast("Couldn't stage those lines", { error: true, detail: String(e) });
          return null;
        });
        if (!res) break;
        if (hi === Math.min(...byHunk.keys())) toast(`${unstage ? "Unstaged" : "Staged"} ${plural(n, "line")}`, { detail: res.cmd });
      }
      ch.lines.clear();
      ch.diffKey = "";
      return refresh({ quiet: true });
    }
    case "clear-lines":
      ch.lines.clear();
      return drawDiff();
    case "ours":
    case "theirs": {
      const ours = what === "ours";
      return run(`Took ${ours ? "your" : "their"} version of ${target.path}`, "take_side", { path: target.path, ours });
    }
    case "resolved": return run(`Marked ${target.path} resolved`, "stage", { paths: [target.path] });
    case "continue": return run("Continued", "op_continue");
    case "abort": {
      const ok = await ask({
        title: "Abort?",
        text: "This stops the operation and puts everything back the way it was before it started.",
        buttons: [{ label: "Abort", value: true, kind: "danger" }],
      });
      if (ok) return run("Aborted", "op_abort");
    }
  }
}

function onLineClick(e, el) {
  if (!el.classList.contains("pick")) return;
  const key = el.dataset.line;
  if (e.shiftKey && ch.anchor && ch.anchor.split(":")[0] === key.split(":")[0]) {
    const [hi, a] = ch.anchor.split(":").map(Number);
    const b = Number(key.split(":")[1]);
    const hunk = ch.diff[0].hunks[hi];
    for (let i = Math.min(a, b); i <= Math.max(a, b); i++) {
      if (hunk.lines[i].kind === "Added" || hunk.lines[i].kind === "Removed") ch.lines.add(`${hi}:${i}`);
    }
  } else if (ch.lines.has(key)) ch.lines.delete(key);
  else ch.lines.add(key);
  ch.anchor = key;
  drawDiff();
}

addPage("changes", {
  title: "Changes",
  icon: "changes",
  full: true,
  badge: (o) => {
    const n = o.counts.conflicts || o.counts.staged + o.counts.unstaged;
    return n ? `<span class="badge ${o.counts.conflicts ? "warn" : "info"}">${n}</span>` : "";
  },
  render: (o) => {
    fixSelection(o);
    return `<div class="changes">
      <div class="files">${opBanner(o)}<div class="files-scroll" id="files">${listHtml(o)}</div>${commitBox(o)}</div>
      <div class="diff-pane" id="diff-pane">${diffHtml()}</div>
    </div>`;
  },
  mounted: (o, view) => {
    updateCommitButton(o);
    $("#c-amend").addEventListener("change", async () => {
      if ($("#c-amend").checked && !$("#c-summary").value.trim()) {
        const msg = await invoke("last_message").catch(() => "");
        const [summary, ...rest] = msg.split("\n");
        $("#c-summary").value = summary;
        $("#c-body").value = rest.join("\n").trim();
      }
      updateCommitButton(state.overview);
    });
    $("#c-go").addEventListener("click", doCommit);
    for (const el of [$("#c-summary"), $("#c-body")]) {
      el.addEventListener("keydown", (e) => {
        if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
          e.preventDefault();
          doCommit();
        }
      });
    }
    view.addEventListener("click", (e) => {
      const act = e.target.closest("[data-ch]");
      if (act) {
        e.stopPropagation();
        if (!act.disabled) onAction(act.dataset.ch, act);
        return;
      }
      const line = e.target.closest(".dl");
      if (line) return onLineClick(e, line);
      const row = e.target.closest(".frow");
      if (row) select(row.dataset.path, row.dataset.section, row.dataset.untracked === "true");
    });
    loadDiff();
  },
  update: (o) => {
    const moved = fixSelection(o);
    const scroller = $("#files");
    const top = scroller.scrollTop;
    scroller.innerHTML = listHtml(o);
    scroller.scrollTop = top;
    const banner = document.querySelector(".op-banner");
    const fresh = opBanner(o);
    if (banner) banner.outerHTML = fresh || "";
    else if (fresh) scroller.insertAdjacentHTML("beforebegin", fresh);
    updateCommitButton(o);
    if (moved) drawDiff();
    loadDiff();
  },
  // ↑/↓ (or j/k) move through files; space stages or unstages.
  key: (e) => {
    const rows = [...document.querySelectorAll(".frow")];
    if (!rows.length) return false;
    const i = rows.findIndex((r) => r.classList.contains("on"));
    const pick = (r) => {
      select(r.dataset.path, r.dataset.section, r.dataset.untracked === "true");
      r.scrollIntoView({ block: "nearest" });
    };
    if (e.key === "ArrowDown" || e.key === "j") pick(rows[Math.min(rows.length - 1, i + 1)]);
    else if (e.key === "ArrowUp" || e.key === "k") pick(rows[Math.max(0, i - 1)]);
    else if (e.key === " " && i >= 0 && rows[i].dataset.section !== "conflicts") onAction(rows[i].dataset.section === "staged" ? "unstage" : "stage", rows[i]);
    else return false;
    return true;
  },
});

ACTIONS.changes = { label: "Open Changes", run: () => go("changes") };
ACTIONS.commit = {
  label: "Commit",
  run: () => {
    go("changes");
    $("#c-summary")?.focus();
  },
};
