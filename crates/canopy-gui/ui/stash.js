// Stash: put work aside and bring it back.
"use strict";

const st = {
  list: [],
  sel: null, // stash name, like stash@{0}
  diff: null,
  count: -1, // stash count at the last load, to know when to reload
  loaded: false,
};

function stashListHtml() {
  if (!st.loaded) return `<div class="diff-empty">Loading…</div>`;
  if (!st.list.length) {
    return `<div class="clean"><div class="clean-mark">⤓</div><b>No stashes</b>
      <p>Stashing puts your uncommitted changes aside so you can switch tasks, then bring them back later.</p></div>`;
  }
  return `<ul class="flist stashes">${st.list.map((s) => `<li class="srow${st.sel === s.name ? " on" : ""}" data-name="${esc(s.name)}">
    <span class="sname mono">${esc(s.name)}</span><span class="smsg">${esc(s.message)}</span><span class="htime">${ago(s.time)}</span></li>`).join("")}</ul>`;
}

function stashDiffHtml() {
  const s = st.list.find((x) => x.name === st.sel);
  if (!s) return `<div class="diff-empty">Select a stash to see what's in it.</div>`;
  return `<div class="diff-head"><b class="mono">${esc(s.name)}</b><span class="faint">${esc(s.message)}</span><span class="grow"></span>
      <button class="btn small" data-s="drop">Drop…</button><button class="btn small" data-s="apply">Apply</button>
      <button class="btn small primary" data-s="pop">Pop</button></div>
    <div class="diff-body">${st.diff ? readonlyDiff(st.diff) : `<div class="diff-empty">Loading…</div>`}</div>
    <div class="diff-tip"><b>Apply</b> brings the changes back and keeps the stash; <b>Pop</b> brings them back and removes it.</div>`;
}

function drawStash() {
  const l = $("#slist");
  if (!l) return;
  l.innerHTML = stashListHtml();
  $("#sdiff").innerHTML = stashDiffHtml();
}

async function loadStashes() {
  const refs = await invoke("refs").catch(() => ({ tags: [], stashes: [] }));
  st.list = refs.stashes;
  st.count = st.list.length;
  st.loaded = true;
  if (!st.list.some((s) => s.name === st.sel)) {
    st.sel = st.list[0]?.name || null;
    st.diff = null;
  }
  drawStash();
  if (st.sel && !st.diff) loadStashDiff();
}

async function loadStashDiff() {
  const name = st.sel;
  const diff = await invoke("stash_diff", { name }).catch((e) => {
    toast(String(e), { error: true });
    return [];
  });
  if (st.sel !== name) return;
  st.diff = diff;
  drawStash();
}

async function stashAction(what, el) {
  const o = state.overview;
  switch (what) {
    case "new": {
      if (!o.counts.staged && !o.counts.unstaged) return toast("Nothing to stash: the working tree is clean.");
      const f = await prompt({ title: "Stash changes", text: "Puts your uncommitted changes aside and leaves a clean working tree.", fields: [
        { id: "message", label: "Name (optional)", placeholder: "half-done refactor" },
        { id: "untracked", label: "Include new (untracked) files", type: "checkbox", value: o.counts.untracked > 0 },
      ], ok: "Stash" });
      if (f) {
        st.sel = null;
        await run("Stashed your changes", "git_op", { op: "stash", args: [f.message, f.untracked ? "untracked" : ""] });
        return loadStashes();
      }
      return;
    }
    case "apply":
    case "pop":
      await run(`${what === "pop" ? "Popped" : "Applied"} ${st.sel}`, "git_op", { op: `stash-${what}`, args: [st.sel] });
      return loadStashes();
    case "drop": {
      const ok = await ask({ title: `Drop ${st.sel}?`, text: "Deletes this stash and the changes in it.", buttons: [{ label: "Drop", value: true, kind: "danger" }] });
      if (ok) {
        await run(`Dropped ${st.sel}`, "git_op", { op: "stash-drop", args: [st.sel] });
        return loadStashes();
      }
    }
  }
}

addPage("stash", {
  title: "Stash",
  icon: "stash",
  full: true,
  badge: (o) => (o.stashes ? `<span class="badge">${o.stashes}</span>` : ""),
  render: () => `<div class="changes">
      <div class="files"><div class="files-scroll" id="slist">${stashListHtml()}</div>
        <div class="commit-box"><button class="btn primary" data-s="new">Stash changes…</button></div></div>
      <div class="diff-pane" id="sdiff">${stashDiffHtml()}</div>
    </div>`,
  mounted: (o, view) => {
    st.loaded = false;
    drawStash();
    view.addEventListener("click", (e) => {
      const act = e.target.closest("[data-s]");
      if (act) return stashAction(act.dataset.s, act);
      const row = e.target.closest(".srow");
      if (row) {
        st.sel = row.dataset.name;
        st.diff = null;
        drawStash();
        loadStashDiff();
      }
    });
    loadStashes();
  },
  update: (o) => {
    if (o.stashes !== st.count) loadStashes();
  },
});
