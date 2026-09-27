// History: the commit graph, search, and a details pane with actions.
"use strict";

const hist = {
  data: null, // { commits, graph, more }
  limit: 200,
  all: false,
  search: "",
  sel: null, // selected oid
  details: null, // { header, files } for sel
  headOid: "", // HEAD when `data` was loaded, to know when to reload
  loading: 0,
};

const ROW_H = 34;
const LANE_W = 14;
const laneX = (i) => 11 + i * LANE_W;

function graphSvg(row, width, merge) {
  const h = ROW_H;
  const mid = h / 2;
  const w = laneX(width - 1) + 11;
  let paths = "";
  const seg = (a, b, y0, y1, lane) => {
    const d = a === b
      ? `M${laneX(a)} ${y0}V${y1}`
      : `M${laneX(a)} ${y0}C${laneX(a)} ${(y0 + y1) / 2} ${laneX(b)} ${(y0 + y1) / 2} ${laneX(b)} ${y1}`;
    paths += `<path d="${d}" class="lane l${lane % 6}"/>`;
  };
  for (const [a, b] of row.up) seg(a, b, 0, mid, a);
  for (const [a, b] of row.down) seg(a, b, mid, h, b);
  const x = laneX(row.col);
  const dot = merge
    ? `<circle cx="${x}" cy="${mid}" r="4.5" class="dot hollow l${row.col % 6}"/>`
    : `<circle cx="${x}" cy="${mid}" r="4.5" class="dot l${row.col % 6}"/>`;
  return `<svg class="graph" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}" aria-hidden="true">${paths}${dot}</svg>`;
}

function histRow(c, i, width) {
  const row = hist.data.graph[i];
  const graph = row ? graphSvg(row, width, c.parents.length > 1) : "";
  return `<li class="hrow${hist.sel === c.oid ? " on" : ""}" data-oid="${esc(c.oid)}">
    ${graph}<span class="hsubj">${refChips(c.refs)}${esc(c.subject)}</span>
    <span class="hauthor">${esc(c.author)}</span><span class="htime">${ago(c.time)}</span><span class="oid">${esc(c.short)}</span>
  </li>`;
}

function histListHtml() {
  const d = hist.data;
  if (!d) return `<div class="diff-empty">Loading…</div>`;
  if (!d.commits.length) return `<div class="diff-empty">${hist.search ? "No commits match." : "No commits yet."}</div>`;
  const width = Math.max(1, ...d.graph.map((r) => r.width));
  const more = d.more ? `<li class="more"><button class="btn small" data-h="more">Load more</button></li>` : "";
  return `<ul class="hlist">${d.commits.map((c, i) => histRow(c, i, width)).join("")}${more}</ul>`;
}

function detailsHtml() {
  const c = hist.data?.commits.find((x) => x.oid === hist.sel);
  if (!c) return `<div class="diff-empty">Select a commit to see what it changed.</div>`;
  const d = hist.details;
  // The header's message lines are indented by 4 spaces after a blank line.
  const msg = d ? d.header.split("\n").filter((l) => l.startsWith("    ")).map((l) => l.slice(4)).join("\n").trim() : c.subject;
  const [title, ...body] = msg.split("\n");
  const parents = c.parents.map((p) => `<button class="link mono" data-h="goto" data-oid="${esc(p)}">${esc(p.slice(0, 7))}</button>`).join(" ");
  return `<div class="cdetails">
    <div class="chead">
      <h3 class="selectable">${esc(title)}</h3>
      ${body.join("\n").trim() ? `<pre class="cbody selectable">${esc(body.join("\n").trim())}</pre>` : ""}
      <div class="cmeta">
        <span><b>${esc(c.author)}</b> <span class="faint">&lt;${esc(c.email)}&gt;</span></span>
        <span class="faint">${new Date(c.time * 1000).toLocaleString()}</span>
        <span class="mono selectable">${esc(c.oid)}</span>
        ${c.parents.length ? `<span class="faint">${c.parents.length > 1 ? "Parents" : "Parent"} ${parents}</span>` : `<span class="faint">First commit</span>`}
      </div>
      <div class="cactions">
        <button class="btn small" data-h="checkout">Check out</button>
        <button class="btn small" data-h="branch">New branch here…</button>
        <button class="btn small" data-h="tag">Tag…</button>
        <button class="btn small" data-h="cherry-pick">Cherry-pick</button>
        <button class="btn small" data-h="revert">Revert</button>
        <button class="btn small" data-h="reset">Reset to here…</button>
      </div>
    </div>
    <div class="cfiles">${d ? readonlyDiff(d.files) : `<div class="diff-empty">Loading…</div>`}</div>
  </div>`;
}

async function loadHistory() {
  const token = ++hist.loading;
  try {
    const data = await invoke("history", { limit: hist.limit, all: hist.all, search: hist.search || null });
    if (token !== hist.loading) return;
    hist.data = data;
    hist.headOid = state.overview.status.branch.oid || "";
    if (!hist.sel || !data.commits.some((c) => c.oid === hist.sel)) {
      hist.sel = data.commits[0]?.oid || null;
      hist.details = null;
    }
    drawHistList();
    if (!hist.details) loadDetails();
  } catch (e) {
    toast(String(e), { error: true });
  }
}

async function loadDetails() {
  const oid = hist.sel;
  drawDetails();
  if (!oid) return;
  const d = await invoke("commit_details", { rev: oid }).catch((e) => ({ header: "", files: [], error: String(e) }));
  if (hist.sel !== oid) return;
  hist.details = d;
  drawDetails();
}

function drawHistList() {
  const el = $("#hlist");
  if (!el) return;
  const top = el.scrollTop;
  el.innerHTML = histListHtml();
  el.scrollTop = top;
}

function drawDetails() {
  const el = $("#hdetails");
  if (el) el.innerHTML = detailsHtml();
}

function selectCommit(oid) {
  hist.sel = oid;
  hist.details = null;
  for (const r of document.querySelectorAll(".hrow")) r.classList.toggle("on", r.dataset.oid === oid);
  loadDetails();
}

async function histAction(what, el) {
  const c = hist.data?.commits.find((x) => x.oid === hist.sel);
  switch (what) {
    case "more":
      hist.limit += 200;
      return loadHistory();
    case "goto": {
      const oid = el.dataset.oid;
      if (hist.data.commits.some((x) => x.oid === oid)) {
        selectCommit(oid);
        document.querySelector(`.hrow[data-oid="${oid}"]`)?.scrollIntoView({ block: "nearest" });
      }
      return;
    }
  }
  if (!c) return;
  const short = c.short;
  switch (what) {
    case "checkout": {
      const ok = await ask({
        title: `Check out ${short}?`,
        text: "You'll be on a detached HEAD: look around, build, test. To keep new work, create a branch.",
        buttons: [{ label: "Check out", value: true, kind: "primary" }],
      });
      if (ok) return run(`Checked out ${short}`, "git_op", { op: "checkout", args: [c.oid] });
      return;
    }
    case "branch": {
      const f = await prompt({ title: `New branch at ${short}`, fields: [
        { id: "name", label: "Branch name", placeholder: "feature/my-idea" },
        { id: "switch", label: "Switch to it", type: "checkbox", value: true },
      ], ok: "Create branch" });
      if (f?.name) return run(`Created ${f.name}`, "git_op", { op: "create-branch", args: [f.name, c.oid, f.switch ? "switch" : ""] });
      return;
    }
    case "tag": {
      const f = await prompt({ title: `Tag ${short}`, fields: [
        { id: "name", label: "Tag name", placeholder: "v1.2.0" },
        { id: "message", label: "Message (leave empty for a lightweight tag)", placeholder: "" },
      ], ok: "Create tag" });
      if (f?.name) return run(`Tagged ${short} as ${f.name}`, "git_op", { op: "create-tag", args: [f.name, c.oid, f.message] });
      return;
    }
    case "cherry-pick":
      return run(`Cherry-picked ${short}`, "git_op", { op: "cherry-pick", args: [c.oid] });
    case "revert": {
      const ok = await ask({
        title: `Revert ${short}?`,
        text: "This makes a new commit that undoes this commit's changes. History is kept.",
        buttons: [{ label: "Revert", value: true, kind: "primary" }],
      });
      if (ok) return run(`Reverted ${short}`, "git_op", { op: "revert", args: [c.oid] });
      return;
    }
    case "reset": {
      const mode = await ask({
        title: `Reset ${state.overview.status.branch.head || "HEAD"} to ${short}?`,
        html: `<p>Moves the branch back to this commit. The commits after it leave the branch (Undo or the reflog can bring them back).</p>
          <ul class="modal-list"><li><b>Soft</b>: their changes stay staged.</li><li><b>Mixed</b>: their changes stay, unstaged.</li>
          <li><b>Hard</b>: their changes <em>and your uncommitted work</em> are thrown away.</li></ul>`,
        buttons: [
          { label: "Soft", value: "reset-soft" },
          { label: "Mixed", value: "reset-mixed", kind: "primary" },
          { label: "Hard", value: "reset-hard", kind: "danger" },
        ],
      });
      if (!mode) return;
      if (mode === "reset-hard" && !(await ask({
        title: "Throw away uncommitted work?",
        text: "A hard reset deletes your uncommitted changes. They can't be recovered.",
        buttons: [{ label: "Hard reset", value: true, kind: "danger" }],
      }))) return;
      return run(`Reset to ${short}`, "git_op", { op: mode, args: [c.oid] });
    }
  }
}

addPage("history", {
  title: "History",
  icon: "history",
  full: true,
  render: () => `<div class="history">
      <div class="hleft">
        <div class="htools">
          <input class="input" id="hsearch" placeholder="Search messages, or @name for an author" value="${esc(hist.search)}" autocomplete="off" spellcheck="false">
          <label class="check"><input type="checkbox" id="hall"${hist.all ? " checked" : ""}> All branches</label>
        </div>
        <div class="hscroll" id="hlist">${histListHtml()}</div>
      </div>
      <div class="hdetails" id="hdetails">${detailsHtml()}</div>
    </div>`,
  mounted: (o, view) => {
    let timer;
    $("#hsearch").addEventListener("input", (e) => {
      clearTimeout(timer);
      timer = setTimeout(() => {
        hist.search = e.target.value;
        hist.limit = 200;
        loadHistory();
      }, 250);
    });
    $("#hall").addEventListener("change", (e) => {
      hist.all = e.target.checked;
      loadHistory();
    });
    view.addEventListener("click", (e) => {
      const act = e.target.closest("[data-h]");
      if (act) return histAction(act.dataset.h, act);
      const row = e.target.closest(".hrow");
      if (row) selectCommit(row.dataset.oid);
    });
    loadHistory();
  },
  // Reload only when HEAD or the refs moved.
  update: (o) => {
    if ((o.status.branch.oid || "") !== hist.headOid) loadHistory();
  },
  key: (e) => {
    const rows = [...document.querySelectorAll(".hrow")];
    const i = rows.findIndex((r) => r.classList.contains("on"));
    let r;
    if (e.key === "ArrowDown" || e.key === "j") r = rows[Math.min(rows.length - 1, i + 1)];
    else if (e.key === "ArrowUp" || e.key === "k") r = rows[Math.max(0, i - 1)];
    else if (e.key === "/") { $("#hsearch").focus(); return true; }
    if (!r) return false;
    selectCommit(r.dataset.oid);
    r.scrollIntoView({ block: "nearest" });
    return true;
  },
});

ACTIONS.history = { label: "Open History", run: () => go("history") };
