// GitHub: pull requests, issues and Actions, through the gh CLI.
"use strict";

const gh = {
  status: null, // from gh_status, per open repo
  statusFor: "",
  home: null, // { branch_pr, review_requests }
  homeAt: 0,
  prs: { filter: "open", list: null, sel: null, detail: null },
  issues: { filter: "open", list: null, sel: null, detail: null },
  runs: { all: false, list: null, sel: null, jobs: null, log: null, loadedAt: 0 },
  // Notifications: every repository's by default, unread only. `unread` is
  // the sidebar badge (every repository), refreshed every two minutes.
  notifs: { everywhere: true, includeRead: false, list: null, sel: null, unread: 0, countAt: 0 },
  ai: null, // { installed, chosen } from ai_status
  failure: undefined, // latest_failure for HEAD (null = none), undefined = not checked
  failureFor: "",
  failureAt: 0,
};

async function aiStatus() {
  if (!gh.ai) gh.ai = await invoke("ai_status").catch(() => ({ installed: [], chosen: null }));
  return gh.ai;
}

/// A popup with everything Canopy knows about why run `id` failed: the same
/// text the AI assistant gets, saved in .git/canopy/fix-ci.md.
async function showDetails(id) {
  const wrap = document.createElement("div");
  wrap.className = "modal-wrap";
  wrap.innerHTML = `<div class="modal details" role="dialog" aria-modal="true" aria-label="Failure details">
    <div class="details-head"><div><h3>What went wrong</h3><p class="faint" id="det-headline">Gathering the failure from GitHub…</p></div>
      <button class="btn icon ghost" data-d="close" type="button" title="Close (Esc)">${icon("x", 14)}</button></div>
    <pre class="details-text selectable" id="det-text"></pre>
    <div class="details-foot"><span class="faint mono" id="det-file"></span><span class="grow"></span>
      <button class="btn small" data-d="copy" type="button">Copy</button>
      <button class="btn small" data-d="editor" type="button">Open in editor</button>
      <button class="btn small" data-d="github" type="button">${brand("github")}View in GitHub</button>
      <button class="btn small primary" data-d="fix" type="button">${assistantMark(gh.ai?.chosen)} Fix with ${esc(gh.ai?.chosen || "AI")}</button></div>
  </div>`;
  let details = null;
  const close = () => {
    wrap.remove();
    document.removeEventListener("keydown", onKey, true);
  };
  const onKey = (e) => {
    if (e.key === "Escape") { e.stopPropagation(); close(); }
  };
  wrap.addEventListener("click", async (e) => {
    if (e.target === wrap) return close();
    const b = e.target.closest("[data-d]");
    if (!b) return;
    switch (b.dataset.d) {
      case "close": return close();
      case "copy":
        try { await navigator.clipboard.writeText(details?.text || ""); b.textContent = "Copied"; } catch { toast("Select the text and copy it."); }
        return;
      case "editor":
        return invoke("open_details").catch((err) => toast(String(err), { error: true }));
      case "github":
        return details?.url && openUrl(details.url);
      case "fix":
        close();
        return fixWithAi(id);
    }
  });
  document.addEventListener("keydown", onKey, true);
  document.body.append(wrap);
  aiStatus().then((ai) => { const f = wrap.querySelector('[data-d="fix"]'); if (f) f.innerHTML = `${assistantMark(ai.chosen)} Fix with ${esc(ai.chosen || "AI")}`; });
  try {
    details = await invoke("failure_details", { id });
    $("#det-headline").textContent = details.headline;
    $("#det-text").textContent = details.text;
    $("#det-file").textContent = details.file.replace(/^.*?(\.git[\/\\])/, "$1");
  } catch (err) {
    $("#det-headline").textContent = "Couldn't get the details";
    $("#det-text").textContent = String(err);
  }
}

/// Write the prompt for run `id` and open the AI assistant on it.
async function fixWithAi(id) {
  const ai = await aiStatus();
  if (!ai.chosen) {
    toast("No AI assistant set up yet: install Claude Code or Codex, or pick one in Settings.");
    return openSettings("general");
  }
  toast(`Asking ${ai.chosen}… gathering the failure from GitHub`);
  try {
    const r = await invoke("fix_with_ai", { id });
    toast(`Opened ${r.assistant} in ${r.opened}`, { detail: state.settings.show_commands ? `prompt: ${r.prompt_file}` : "" });
  } catch (e) {
    toast("Couldn't open the AI assistant", { error: true, detail: String(e) });
  }
}

const openUrl = (url) => invoke("open_url", { url }).catch((e) => toast(String(e), { error: true }));

async function ghReady() {
  const root = state.overview?.root;
  if (gh.statusFor !== root) {
    gh.statusFor = root;
    gh.status = null;
    gh.home = null;
    gh.prs.list = gh.issues.list = gh.runs.list = gh.notifs.list = null;
    gh.notifs.countAt = 0;
  }
  if (!gh.status) gh.status = await invoke("gh_status").catch(() => ({ state: "not_installed" }));
  return gh.status.state === "ready";
}

function setupCard() {
  const s = gh.status?.state;
  if (!s) return `<div class="diff-empty">Checking GitHub…</div>`;
  const mac = navigator.platform.includes("Mac");
  const body = {
    not_installed: `<h3>Connect GitHub</h3><p>Canopy uses the GitHub CLI (<code>gh</code>) with your own login. Install it, then log in:</p>
      <pre class="codeblock selectable">${mac ? "brew install gh" : /Win/.test(navigator.platform) ? "winget install --id GitHub.cli -e" : "# see https://cli.github.com for your distribution"}
gh auth login</pre>`,
    not_logged_in: `<h3>Log in to GitHub</h3><p>The GitHub CLI is installed but not logged in. In a terminal, run:</p><pre class="codeblock selectable">gh auth login</pre>`,
    not_github: `<h3>Not a GitHub repository</h3><p>This repository has no GitHub remote, so there are no pull requests, issues or runs to show.
      Everything on the git pages still works.</p>`,
  }[s];
  return `<div class="setup card">${body}<div class="setup-actions">
    ${s === "not_github" ? "" : `<button class="btn" data-g="open" data-url="https://cli.github.com">GitHub CLI website</button>`}
    <button class="btn primary" data-g="retry">Try again</button></div></div>`;
}

function checkIcon(state) {
  const map = { passed: ["✓", "green", "Checks passed"], failed: ["✗", "red", "Checks failing"], pending: ["●", "amber", "Checks running"], neutral: ["–", "", "Skipped"] };
  const [g, cls, title] = map[state] || ["", "", ""];
  return g ? `<span class="ck ${cls}" title="${title}">${g}</span>` : `<span class="ck"></span>`;
}

const REVIEW = {
  APPROVED: `<span class="pill green">approved</span>`,
  CHANGES_REQUESTED: `<span class="pill red">changes requested</span>`,
  REVIEW_REQUIRED: `<span class="pill">review required</span>`,
};

function stateBadge(item, kind) {
  if (item.isDraft) return `<span class="pill">draft</span>`;
  const st = item.state;
  const cls = st === "OPEN" ? "green" : st === "MERGED" ? "violet" : "red";
  return `<span class="pill ${cls}">${st.toLowerCase()}</span>`;
}

function labelChips(labels) {
  return labels.map((l) => `<span class="label" style="--c:#${esc(l.color || "888888")}">${esc(l.name)}</span>`).join("");
}

function textBlock(text) {
  const t = (text || "").trim();
  return t ? `<div class="md selectable">${esc(t)}</div>` : `<div class="faint">No description.</div>`;
}

function comments(list) {
  if (!list?.length) return "";
  return `<h4>Comments <span class="badge">${list.length}</span></h4>` + list
    .map((c) => `<div class="comment"><div class="comment-h"><b>${esc(c.author.login)}</b><span class="faint">${ago(c.createdAt)}</span></div>${textBlock(c.body)}</div>`)
    .join("");
}

function filterTabs(ns, tabs, extra = "") {
  return `<div class="tabs">${tabs.map(([id, l]) => `<button class="tab${gh[ns].filter === id ? " on" : ""}" data-g="filter" data-ns="${ns}" data-f="${id}">${l}</button>`).join("")}<span class="grow"></span>${extra}</div>`;
}

// ---------------------------------------------------------------- pull requests

function prListHtml() {
  const p = gh.prs;
  if (!p.list) return `<div class="diff-empty">Loading…</div>`;
  if (!p.list.length) return `<div class="diff-empty">No pull requests here.</div>`;
  return `<ul class="glist">${p.list.map((pr) => `<li class="grow-row${p.sel === pr.number ? " on" : ""}" data-n="${pr.number}">
      ${checkIcon(pr.check_state)}<div class="gmain"><div class="gtitle"><span class="num">#${pr.number}</span> ${esc(pr.title)}</div>
      <div class="gsub"><span>${esc(pr.author.login)}</span><span class="mono">${esc(pr.headRefName)} → ${esc(pr.baseRefName)}</span><span>${ago(pr.updatedAt)}</span></div></div>
      ${pr.isDraft ? `<span class="pill">draft</span>` : REVIEW[pr.reviewDecision] || ""}
    </li>`).join("")}</ul>`;
}

function prDetailHtml() {
  const p = gh.prs;
  const pr = p.detail;
  if (!p.sel) return `<div class="diff-empty">Select a pull request.</div>`;
  if (!pr) return `<div class="diff-empty">Loading…</div>`;
  const checks = pr.statusCheckRollup.length
    ? `<h4>Checks <span class="faint">${pr.checks.passed}/${pr.checks.total} passed</span></h4><ul class="checks">${pr.statusCheckRollup
        .map((c) => {
          const st = (c.conclusion || c.state || c.status || "").toUpperCase();
          const s = st === "SUCCESS" ? "passed" : /FAIL|ERROR|TIMED_OUT|CANCELLED/.test(st) ? "failed" : /SKIP|NEUTRAL/.test(st) ? "neutral" : "pending";
          return `<li>${checkIcon(s)}<span>${esc(c.workflowName ? `${c.workflowName} / ${c.name}` : c.name)}</span>${c.detailsUrl ? `<button class="link" data-g="open" data-url="${esc(c.detailsUrl)}">details</button>` : ""}</li>`;
        })
        .join("")}</ul>`
    : "";
  const reviews = pr.reviews.filter((r) => r.state !== "COMMENTED" || r.body);
  const open = pr.state === "OPEN";
  return `<div class="gdetail">
    <h2 class="selectable">${esc(pr.title)} <span class="num">#${pr.number}</span></h2>
    <div class="gmeta">${stateBadge(pr)} <span><b>${esc(pr.author.login)}</b> wants to merge <code>${esc(pr.headRefName)}</code> into <code>${esc(pr.baseRefName)}</code></span>
      <span class="adds">+${pr.additions}</span><span class="dels">−${pr.deletions}</span>${REVIEW[pr.reviewDecision] || ""}${labelChips(pr.labels)}</div>
    <div class="cactions">
      <button class="btn small" data-g="open" data-url="${esc(pr.url)}">${brand("github")}View in GitHub</button>
      <button class="btn small" data-g="pr-checkout">Check out</button>
      <button class="btn small" data-g="pr-comment">Comment…</button>
      ${open ? `<button class="btn small" data-g="pr-review">Review…</button><button class="btn small" data-g="pr-close">Close…</button>
        <button class="btn small primary" data-g="pr-merge"${pr.mergeable === "CONFLICTING" ? ` title="This branch has conflicts with ${esc(pr.baseRefName)}"` : ""}>Merge…</button>` : ""}
    </div>
    ${pr.mergeable === "CONFLICTING" && open ? `<div class="op-banner"><b>Conflicts with ${esc(pr.baseRefName)}.</b><span>Check it out, merge ${esc(pr.baseRefName)} into it, resolve in Changes, then push.</span></div>` : ""}
    ${textBlock(pr.body)}
    ${checks}
    ${reviews.length ? `<h4>Reviews</h4>${reviews.map((r) => `<div class="comment"><div class="comment-h"><b>${esc(r.author.login)}</b>${REVIEW[r.state] || `<span class="pill">${esc(r.state.toLowerCase())}</span>`}<span class="faint">${ago(r.submittedAt)}</span></div>${r.body ? textBlock(r.body) : ""}</div>`).join("")}` : ""}
    ${comments(pr.comments)}
  </div>`;
}

async function loadPrs() {
  if (!(await ghReady())) return drawGh("prs");
  try {
    gh.prs.list = await invoke("gh_prs", { filter: gh.prs.filter });
  } catch (e) {
    gh.prs.list = [];
    toast(String(e), { error: true });
  }
  if (!gh.prs.list.some((p) => p.number === gh.prs.sel)) {
    gh.prs.sel = gh.prs.list[0]?.number || null;
    gh.prs.detail = null;
  }
  drawGh("prs");
  if (gh.prs.sel && !gh.prs.detail) loadPr();
}

async function loadPr() {
  const n = gh.prs.sel;
  const d = await invoke("gh_pr", { number: n }).catch((e) => (toast(String(e), { error: true }), null));
  if (gh.prs.sel !== n) return;
  gh.prs.detail = d;
  drawGh("prs");
}

// ---------------------------------------------------------------- issues

function issueListHtml() {
  const s = gh.issues;
  if (!s.list) return `<div class="diff-empty">Loading…</div>`;
  if (!s.list.length) return `<div class="diff-empty">No issues here.</div>`;
  return `<ul class="glist">${s.list.map((i) => `<li class="grow-row${s.sel === i.number ? " on" : ""}" data-n="${i.number}">
      <span class="ck ${i.state === "OPEN" ? "green" : "violet"}">${i.state === "OPEN" ? "○" : "✓"}</span>
      <div class="gmain"><div class="gtitle"><span class="num">#${i.number}</span> ${esc(i.title)}</div>
      <div class="gsub"><span>${esc(i.author.login)}</span><span>${ago(i.updatedAt)}</span>${i.comments.length ? `<span>💬 ${i.comments.length}</span>` : ""}${labelChips(i.labels)}</div></div>
    </li>`).join("")}</ul>`;
}

function issueDetailHtml() {
  const s = gh.issues;
  const i = s.detail;
  if (!s.sel) return `<div class="diff-empty">Select an issue.</div>`;
  if (!i) return `<div class="diff-empty">Loading…</div>`;
  const open = i.state === "OPEN";
  return `<div class="gdetail">
    <h2 class="selectable">${esc(i.title)} <span class="num">#${i.number}</span></h2>
    <div class="gmeta">${stateBadge(i)}<span>opened by <b>${esc(i.author.login)}</b></span>${labelChips(i.labels)}</div>
    <div class="cactions">
      <button class="btn small" data-g="open" data-url="${esc(i.url)}">${brand("github")}View in GitHub</button>
      <button class="btn small" data-g="issue-comment">Comment…</button>
      ${open ? `<button class="btn small" data-g="issue-close">Close</button>` : `<button class="btn small" data-g="issue-reopen">Reopen</button>`}
    </div>
    ${textBlock(i.body)}
    ${comments(i.comments)}
  </div>`;
}

async function loadIssues() {
  if (!(await ghReady())) return drawGh("issues");
  try {
    gh.issues.list = await invoke("gh_issues", { filter: gh.issues.filter });
  } catch (e) {
    gh.issues.list = [];
    toast(String(e), { error: true });
  }
  if (!gh.issues.list.some((i) => i.number === gh.issues.sel)) {
    gh.issues.sel = gh.issues.list[0]?.number || null;
    gh.issues.detail = null;
  }
  drawGh("issues");
  if (gh.issues.sel && !gh.issues.detail) loadIssue();
}

async function loadIssue() {
  const n = gh.issues.sel;
  const d = await invoke("gh_issue", { number: n }).catch((e) => (toast(String(e), { error: true }), null));
  if (gh.issues.sel !== n) return;
  gh.issues.detail = d;
  drawGh("issues");
}

// ---------------------------------------------------------------- actions

const RUN_ICON = { passed: "passed", failed: "failed", pending: "pending", neutral: "neutral" };

function runListHtml() {
  const r = gh.runs;
  if (!r.list) return `<div class="diff-empty">Loading…</div>`;
  if (!r.list.length) return `<div class="diff-empty">${r.all ? "No workflow runs yet." : "No runs for this branch. Push it, or show all branches."}</div>`;
  return `<ul class="glist">${r.list.map((run) => `<li class="grow-row${r.sel === run.databaseId ? " on" : ""}" data-n="${run.databaseId}">
      ${checkIcon(RUN_ICON[run.state])}<div class="gmain"><div class="gtitle">${esc(run.displayTitle)}</div>
      <div class="gsub"><span>${esc(run.workflowName)}</span><span class="mono">${esc(run.headBranch)}</span><span>${esc(run.event)}</span><span>${ago(run.createdAt)}</span></div></div>
    </li>`).join("")}</ul>`;
}

function runDetailHtml() {
  const r = gh.runs;
  const run = r.list?.find((x) => x.databaseId === r.sel);
  if (!run) return `<div class="diff-empty">Select a run.</div>`;
  const jobs = r.jobs
    ? r.jobs.map((j) => {
        const st = j.status !== "completed" ? "pending" : j.conclusion === "success" ? "passed" : /skipped|neutral/.test(j.conclusion) ? "neutral" : "failed";
        const steps = j.steps.filter((s) => s.conclusion !== "skipped")
          .map((s) => {
            const ss = s.status !== "completed" ? "pending" : s.conclusion === "success" ? "passed" : "failed";
            return `<li>${checkIcon(ss)}<span>${esc(s.name)}</span></li>`;
          }).join("");
        return `<details class="job"${st === "failed" || st === "pending" ? " open" : ""}><summary>${checkIcon(st)}<b>${esc(j.name)}</b></summary><ul class="checks">${steps}</ul></details>`;
      }).join("")
    : `<div class="diff-empty">Loading jobs…</div>`;
  const log = run.state === "failed"
    ? `<h4>End of the failed log</h4>${r.log == null ? `<div class="faint">Loading…</div>` : `<pre class="log selectable">${esc(r.log) || "(no log)"}</pre>`}`
    : "";
  return `<div class="gdetail">
    <h2 class="selectable">${esc(run.displayTitle)}</h2>
    <div class="gmeta">${checkIcon(RUN_ICON[run.state])}<span>${esc(run.workflowName)} #${run.number}</span><code>${esc(run.headBranch)}</code><span class="faint">${esc(run.event)} · ${ago(run.createdAt)}</span></div>
    <div class="cactions">
      <button class="btn small" data-g="open" data-url="${esc(run.url)}">${brand("github")}View in GitHub</button>
      ${run.state === "failed" ? `<button class="btn small" data-g="details">Details</button>
        <button class="btn small" data-g="run-rerun">Re-run failed jobs</button>
        <button class="btn small primary" data-g="fix-ai" title="Write a prompt from this failure and open your AI assistant with it">${assistantMark(gh.ai?.chosen)} Fix with ${esc(gh.ai?.chosen || "AI")}</button>` : ""}
    </div>
    ${jobs}${log}
  </div>`;
}

async function loadRuns() {
  if (!(await ghReady())) return drawGh("runs");
  await aiStatus();
  try {
    gh.runs.list = await invoke("gh_runs", { all: gh.runs.all });
  } catch (e) {
    gh.runs.list = [];
    toast(String(e), { error: true });
  }
  gh.runs.loadedAt = Date.now();
  if (!gh.runs.list.some((r) => r.databaseId === gh.runs.sel)) {
    gh.runs.sel = gh.runs.list[0]?.databaseId || null;
    gh.runs.jobs = gh.runs.log = null;
  }
  drawGh("runs");
  if (gh.runs.sel) loadRun();
}

async function loadRun() {
  const id = gh.runs.sel;
  const run = gh.runs.list.find((r) => r.databaseId === id);
  const [jobs, log] = await Promise.all([
    invoke("gh_run_jobs", { id }).catch(() => []),
    run?.state === "failed" ? invoke("gh_failed_log", { id }).catch((e) => String(e)) : Promise.resolve(null),
  ]);
  if (gh.runs.sel !== id) return;
  gh.runs.jobs = jobs;
  gh.runs.log = log;
  drawGh("runs");
}

// ---------------------------------------------------------------- notifications

const NOTIF_KIND = {
  PullRequest: ["pr", "Pull request"],
  Issue: ["issue", "Issue"],
  Release: ["actions", "Release"],
  CheckSuite: ["actions", "CI"],
  Discussion: ["issue", "Discussion"],
  Commit: ["history", "Commit"],
};

/// Whether notification `n` is about the repository that's open.
function notifHere(n) {
  return n.repo.toLowerCase() === (gh.status?.repo?.nameWithOwner || "").toLowerCase();
}

function notifListHtml() {
  const s = gh.notifs;
  if (!s.list) return `<div class="diff-empty">Loading…</div>`;
  if (!s.list.length) {
    return `<div class="diff-empty">${s.includeRead ? "No notifications." : "You're all caught up: no unread notifications."}</div>`;
  }
  return `<ul class="glist">${s.list.map((n) => {
      const [ic, label] = NOTIF_KIND[n.kind] || ["bell", n.kind];
      return `<li class="grow-row notif${n.unread ? " unread" : ""}${s.sel === n.id ? " on" : ""}" data-id="${esc(n.id)}">
      <span class="ck" title="${esc(label)}">${icon(ic, 14)}</span>
      <div class="gmain"><div class="gtitle">${n.number ? `<span class="num">#${n.number}</span> ` : ""}${esc(n.title)}</div>
      <div class="gsub">${s.everywhere ? `<span class="mono">${esc(n.repo)}</span>` : ""}<span>${esc(n.why)}</span><span>${ago(n.updated_at)}</span></div></div>
      ${n.unread ? `<span class="dot-unread" title="Unread"></span>` : ""}
    </li>`;
    }).join("")}</ul>`;
}

function notifDetailHtml() {
  const s = gh.notifs;
  const n = s.list?.find((x) => x.id === s.sel);
  if (!n) return `<div class="diff-empty">${s.list?.length ? "Select a notification." : ""}</div>`;
  const [, label] = NOTIF_KIND[n.kind] || ["bell", n.kind];
  const inCanopy = notifHere(n) && n.number && (n.kind === "PullRequest" || n.kind === "Issue");
  return `<div class="gdetail">
    <h2 class="selectable">${esc(n.title)}${n.number ? ` <span class="num">#${n.number}</span>` : ""}</h2>
    <div class="gmeta"><span class="pill">${esc(label)}</span>${n.unread ? `<span class="pill green">unread</span>` : `<span class="pill">read</span>`}
      <span class="mono">${esc(n.repo)}</span><span class="faint">${ago(n.updated_at)}</span></div>
    <p class="notif-why">You got this because <b>${esc(n.why)}</b>.</p>
    <div class="cactions">
      ${inCanopy ? `<button class="btn small primary" data-g="notif-show">Open in Canopy</button>` : ""}
      <button class="btn small${inCanopy ? "" : " primary"}" data-g="notif-open">${brand("github")}View in GitHub</button>
      ${n.unread ? `<button class="btn small" data-g="notif-read">Mark as read</button>` : ""}
    </div>
    ${notifHere(n) ? "" : `<p class="faint small">This is about another repository, so it opens on GitHub.</p>`}
  </div>`;
}

async function loadNotifs() {
  if (!(await ghReady())) return drawGh("notifs");
  const s = gh.notifs;
  try {
    s.list = await invoke("gh_notifications", { everywhere: s.everywhere, includeRead: s.includeRead });
  } catch (e) {
    s.list = [];
    toast(String(e), { error: true });
  }
  if (s.everywhere && !s.includeRead) {
    s.unread = s.list.length;
    s.countAt = Date.now();
    renderNav();
  }
  if (!s.list.some((n) => n.id === s.sel)) s.sel = s.list[0]?.id || null;
  drawGh("notifs");
}

/// The sidebar badge: unread notifications in every repository. Refreshed
/// in the background at most every two minutes.
function notifBadge() {
  const s = gh.notifs;
  if (state.overview && Date.now() - s.countAt > 120000) {
    s.countAt = Date.now();
    ghReady().then(async (ready) => {
      if (!ready) return;
      const list = await invoke("gh_notifications", { everywhere: true, includeRead: false }).catch(() => null);
      if (!list) return;
      if (list.length !== s.unread) {
        s.unread = list.length;
        renderNav();
        if (state.page === "home") { delete $("#view").dataset.shown; render(); }
      }
    });
  }
  if (!s.unread) return "";
  return `<span class="badge info" title="Unread notifications, in every repository">${s.unread >= 50 ? "50+" : s.unread}</span>`;
}

/// Mark `ids` read here (without waiting for GitHub), and keep the badge right.
function markLocallyRead(ids) {
  const s = gh.notifs;
  for (const n of s.list || []) {
    if (ids.includes(n.id) && n.unread) {
      n.unread = false;
      s.unread = Math.max(0, s.unread - 1);
    }
  }
  renderNav();
}

// ---------------------------------------------------------------- shared page code

const GH_PAGES = {
  prs: { list: prListHtml, detail: prDetailHtml, load: loadPrs, one: loadPr },
  issues: { list: issueListHtml, detail: issueDetailHtml, load: loadIssues, one: loadIssue },
  runs: { list: runListHtml, detail: runDetailHtml, load: loadRuns, one: loadRun },
  notifs: { list: notifListHtml, detail: notifDetailHtml, load: loadNotifs, one: () => drawGh("notifs") },
};

function ghPageHtml(ns) {
  if (gh.status && gh.status.state !== "ready") return `<div class="setup-wrap">${setupCard()}</div>`;
  if (!gh.status) return `<div class="diff-empty">Checking GitHub…</div>`;
  const tools = {
    prs: filterTabs("prs", [["open", "Open"], ["mine", "Mine"], ["review", "To review"], ["all", "All"]], `<button class="btn small primary" data-g="pr-create">New PR…</button>`),
    issues: filterTabs("issues", [["open", "Open"], ["mine", "Assigned to me"], ["all", "All"]], `<button class="btn small primary" data-g="issue-create">New issue…</button>`),
    runs: `<div class="tabs"><button class="tab${gh.runs.all ? "" : " on"}" data-g="runs-all" data-all="">This branch</button><button class="tab${gh.runs.all ? " on" : ""}" data-g="runs-all" data-all="1">All branches</button></div>`,
    notifs: `<div class="tabs"><button class="tab${gh.notifs.everywhere ? " on" : ""}" data-g="notif-where" data-all="1">All repositories</button><button class="tab${gh.notifs.everywhere ? "" : " on"}" data-g="notif-where" data-all="">This repository</button>
      <span class="grow"></span><label class="check small" title="Also show notifications you've read"><input type="checkbox" data-g="notif-include-read"${gh.notifs.includeRead ? " checked" : ""}> Read too</label>
      <button class="btn small" data-g="notif-read-all">Mark all read</button></div>`,
  }[ns];
  return `<div class="gh">
    <div class="gleft">${tools}<div class="hscroll" id="glist">${GH_PAGES[ns].list()}</div></div>
    <div class="hdetails" id="gdetail">${GH_PAGES[ns].detail()}</div>
  </div>`;
}

function drawGh(ns) {
  if (state.page !== ns) return;
  const view = $("#view");
  const list = $("#glist");
  if (!list || (gh.status && gh.status.state !== "ready")) {
    view.innerHTML = ghPageHtml(ns);
    return;
  }
  const top = list.scrollTop;
  list.innerHTML = GH_PAGES[ns].list();
  list.scrollTop = top;
  const d = $("#gdetail");
  const dt = d.scrollTop;
  d.innerHTML = GH_PAGES[ns].detail();
  d.scrollTop = dt;
}

async function ghAction(ns, what, el) {
  const o = state.overview;
  const pr = gh.prs.detail;
  const issue = gh.issues.detail;
  const after = async (res) => {
    if (!res) return;
    if (ns === "prs") { gh.prs.detail = null; await loadPrs(); }
    if (ns === "issues") { gh.issues.detail = null; await loadIssues(); }
    if (ns === "runs") await loadRuns();
  };
  switch (what) {
    case "open": return openUrl(el.dataset.url);
    case "retry":
      gh.status = null;
      drawGh(ns);
      return GH_PAGES[ns].load();
    case "filter":
      gh[el.dataset.ns].filter = el.dataset.f;
      gh[el.dataset.ns].list = null;
      drawGh(ns);
      return GH_PAGES[ns].load();
    case "runs-all":
      gh.runs.all = !!el.dataset.all;
      gh.runs.list = null;
      drawGh(ns);
      return loadRuns();
    case "pr-create": {
      const head = o.status.branch.head;
      if (!head) return toast("Check out a branch first.");
      if (!o.status.branch.upstream) return toast(`Push ${head} first, then open a pull request for it.`);
      const f = await prompt({ title: `New pull request from ${head}`, fields: [
        { id: "title", label: "Title", value: o.log[0]?.subject || "" },
        { id: "body", label: "Description", placeholder: "What does this change, and why?" },
        { id: "base", label: "Into (leave empty for the default branch)", placeholder: gh.status.repo?.defaultBranchRef?.name || "main" },
        { id: "draft", label: "Open as a draft", type: "checkbox" },
      ], ok: "Create pull request" });
      if (f?.title) return after(await run("Opened a pull request", "gh_op", { op: "pr-create", args: [f.title, f.body, f.base, f.draft ? "draft" : ""] }));
      return;
    }
    case "pr-checkout": return run(`Checked out #${pr.number}`, "gh_op", { op: "pr-checkout", args: [String(pr.number)] });
    case "pr-comment": {
      const f = await prompt({ title: `Comment on #${pr.number}`, fields: [{ id: "body", label: "Comment" }], ok: "Comment" });
      if (f?.body) return after(await run("Commented", "gh_op", { op: "pr-comment", args: [String(pr.number), f.body] }));
      return;
    }
    case "pr-review": {
      const f = await prompt({ title: `Review #${pr.number}`, fields: [
        { id: "kind", label: "Your review", type: "select", value: "approve", options: [["approve", "Approve"], ["request-changes", "Request changes"], ["comment", "Comment only"]] },
        { id: "body", label: "Comment (needed unless approving)" },
      ], ok: "Submit review" });
      if (!f) return;
      if (f.kind !== "approve" && !f.body) return toast("Add a comment to explain what should change.");
      return after(await run("Review submitted", "gh_op", { op: "pr-review", args: [String(pr.number), f.kind, f.body] }));
    }
    case "pr-merge": {
      const f = await prompt({ title: `Merge #${pr.number} into ${pr.baseRefName}?`, fields: [
        { id: "method", label: "How", type: "select", value: "squash", options: [["squash", "Squash and merge (one commit)"], ["merge", "Create a merge commit"], ["rebase", "Rebase and merge"]] },
        { id: "del", label: `Delete ${pr.headRefName} afterwards`, type: "checkbox", value: true },
      ], ok: "Merge" });
      if (f) return after(await run(`Merged #${pr.number}`, "gh_op", { op: "pr-merge", args: [String(pr.number), f.method, f.del ? "delete" : ""] }));
      return;
    }
    case "pr-close": {
      const ok = await ask({ title: `Close #${pr.number} without merging?`, text: "You can reopen it on GitHub later.", buttons: [{ label: "Close pull request", value: true, kind: "danger" }] });
      if (ok) return after(await run(`Closed #${pr.number}`, "gh_op", { op: "pr-close", args: [String(pr.number)] }));
      return;
    }
    case "issue-create": {
      const f = await prompt({ title: "New issue", fields: [{ id: "title", label: "Title" }, { id: "body", label: "Description" }], ok: "Create issue" });
      if (f?.title) return after(await run("Opened an issue", "gh_op", { op: "issue-create", args: [f.title, f.body] }));
      return;
    }
    case "issue-comment": {
      const f = await prompt({ title: `Comment on #${issue.number}`, fields: [{ id: "body", label: "Comment" }], ok: "Comment" });
      if (f?.body) return after(await run("Commented", "gh_op", { op: "issue-comment", args: [String(issue.number), f.body] }));
      return;
    }
    case "issue-close":
    case "issue-reopen":
      return after(await run(what === "issue-close" ? `Closed #${issue.number}` : `Reopened #${issue.number}`, "gh_op", { op: what, args: [String(issue.number)] }));
    case "details":
      return showDetails(gh.runs.sel);
    case "fix-ai":
      return fixWithAi(gh.runs.sel);
    case "run-rerun":
      return after(await run("Re-running failed jobs", "gh_op", { op: "run-rerun", args: [String(gh.runs.sel)] }));
    case "notif-where":
      gh.notifs.everywhere = !!el.dataset.all;
      gh.notifs.list = null;
      drawGh(ns);
      return loadNotifs();
    case "notif-include-read":
      gh.notifs.includeRead = el.checked;
      gh.notifs.list = null;
      drawGh(ns);
      return loadNotifs();
    case "notif-open":
    case "notif-show":
    case "notif-read": {
      const n = gh.notifs.list?.find((x) => x.id === gh.notifs.sel);
      if (!n) return;
      if (what === "notif-open") openUrl(n.url);
      if (what === "notif-show") {
        const page = n.kind === "PullRequest" ? "prs" : "issues";
        Object.assign(gh[page], { filter: "all", list: null, sel: n.number, detail: null });
      }
      if (n.unread) {
        markLocallyRead([n.id]);
        invoke("gh_op", { op: "notif-read", args: [n.id] }).catch((e) => toast("Couldn't mark it read", { error: true, detail: String(e) }));
      }
      if (what === "notif-show") return go(n.kind === "PullRequest" ? "prs" : "issues");
      return drawGh(ns);
    }
    case "notif-read-all": {
      const where = gh.notifs.everywhere ? "in every repository" : "in this repository";
      const ok = await ask({ title: "Mark all notifications read?", text: `Every unread notification ${where} is marked read on GitHub.`, buttons: [{ label: "Mark all read", value: true, kind: "primary" }] });
      if (!ok) return;
      const res = await run("Marked all read", "gh_op", { op: "notif-read-all", args: [gh.notifs.everywhere ? "everywhere" : ""] });
      if (res) {
        markLocallyRead((gh.notifs.list || []).map((n) => n.id));
        if (gh.notifs.everywhere) gh.notifs.unread = 0;
        gh.notifs.countAt = 0;
        renderNav();
        await loadNotifs();
      }
      return;
    }
  }
}

function ghPage(ns, title, icon) {
  return {
    title,
    icon,
    full: true,
    render: () => ghPageHtml(ns),
    mounted: (o, view) => {
      view.addEventListener("click", (e) => {
        const act = e.target.closest("[data-g]");
        if (act) return ghAction(ns, act.dataset.g, act);
        const row = e.target.closest(".grow-row");
        if (!row) return;
        const s = gh[ns];
        if (ns === "notifs") {
          s.sel = row.dataset.id;
          return drawGh(ns);
        }
        const n = Number(row.dataset.n);
        s.sel = n;
        if (ns === "runs") s.jobs = s.log = null;
        else s.detail = null;
        drawGh(ns);
        GH_PAGES[ns].one();
      });
      GH_PAGES[ns].load();
    },
    // Runs change on their own while CI works; refresh them every 15 s.
    update: () => {
      if (ns === "runs" && gh.runs.list && Date.now() - gh.runs.loadedAt > 15000) loadRuns();
    },
  };
}

addPage("prs", ghPage("prs", "Pull requests", "pr"), 1);
addPage("issues", ghPage("issues", "Issues", "issue"), 1);
addPage("runs", ghPage("runs", "Actions", "actions"), 1);
addPage("notifs", { ...ghPage("notifs", "Notifications", "bell"), badge: notifBadge }, 1);

// ---------------------------------------------------------------- Home card

/// "Canopy found an error in your latest commit": shown on Home when CI failed
/// for the commit you're on. Checked when HEAD moves, and every minute.
function ghFailureBanner() {
  const head = state.overview?.status.branch.oid || "";
  if (gh.failureFor !== head || Date.now() - gh.failureAt > 60000) {
    gh.failureFor = head;
    gh.failureAt = Date.now();
    ghReady().then(async (ready) => {
      if (!ready) return;
      const [f] = await Promise.all([invoke("latest_failure").catch(() => null), aiStatus()]);
      const changed = JSON.stringify(f) !== JSON.stringify(gh.failure);
      gh.failure = f;
      if (changed && state.page === "home") {
        delete $("#view").dataset.shown;
        render();
      }
    });
  }
  const f = gh.failure;
  if (!f) return "";
  const who = gh.ai?.chosen;
  return `<div class="card span-12 fail-banner">
    <div class="fail-icon">✗</div>
    <div class="fail-text"><b>Canopy found an error in your latest commit.</b><p>${esc(f.headline)} <span class="faint">(${esc(f.workflow)})</span></p></div>
    <div class="fail-actions">
      <button class="btn small" data-fail="details">Details</button>
      <button class="btn small" data-fail="open" data-url="${esc(f.url)}">${brand("github")}View in GitHub</button>
      ${who ? `<button class="btn small primary" data-fail="fix" title="Write a prompt from this failure and open ${esc(who)} with it">${assistantMark(who)} Fix with ${esc(who)}</button>`
            : `<button class="btn small" data-fail="setup">Set up an AI assistant</button>`}
    </div></div>`;
}

document.addEventListener("click", (e) => {
  const b = e.target.closest("[data-fail]");
  if (!b) return;
  if (b.dataset.fail === "open") openUrl(b.dataset.url);
  else if (b.dataset.fail === "details") showDetails(gh.failure.run_id);
  else if (b.dataset.fail === "fix") fixWithAi(gh.failure.run_id);
  else openSettings("general");
});

/// Drawn by Home from the cache; refreshed in the background every minute.
function ghHomeCard() {
  if (Date.now() - gh.homeAt > 60000) {
    gh.homeAt = Date.now();
    ghReady().then(async (ready) => {
      if (!ready) return;
      gh.home = await invoke("gh_home").catch(() => null);
      if (state.page === "home") {
        delete $("#view").dataset.shown;
        render();
      }
    });
  }
  const h = gh.home;
  const unread = gh.notifs.unread;
  if ((!h || (!h.branch_pr && !h.review_requests)) && !unread) return "";
  const pr = h?.branch_pr;
  const prLine = pr
    ? `<div class="gh-home-row" data-gh-pr="${pr.number}">${checkIcon(pr.check_state)}<span><span class="num">#${pr.number}</span> ${esc(pr.title)}</span>${REVIEW[pr.reviewDecision] || ""}
       <span class="faint">${pr.checks.total ? `${pr.checks.passed}/${pr.checks.total} checks passed` : "no checks"}</span></div>`
    : "";
  const reviews = h?.review_requests
    ? `<div class="gh-home-row" data-gh-reviews>${checkIcon("")}<span>${plural(h.review_requests, "pull request")} waiting for your review</span></div>`
    : "";
  const notifs = unread
    ? `<div class="gh-home-row" data-gh-notifs><span class="ck">${icon("bell", 14)}</span><span>${unread >= 50 ? "50+" : unread} unread notification${unread === 1 ? "" : "s"}</span></div>`
    : "";
  return `<div class="card span-12"><div class="card-h"><h3>GitHub</h3></div><div class="card-b">${prLine}${reviews}${notifs}</div></div>`;
}

document.addEventListener("click", (e) => {
  const pr = e.target.closest("[data-gh-pr]");
  const reviews = e.target.closest("[data-gh-reviews]");
  if (e.target.closest("[data-gh-notifs]")) {
    Object.assign(gh.notifs, { everywhere: true, includeRead: false, list: null });
    return go("notifs");
  }
  if (pr) {
    gh.prs.filter = "open";
    gh.prs.sel = Number(pr.dataset.ghPr);
    gh.prs.detail = null;
    go("prs");
  } else if (reviews) {
    gh.prs.filter = "review";
    gh.prs.list = null;
    go("prs");
  }
});
