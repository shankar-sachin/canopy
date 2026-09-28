// Tests for the desktop app's front end. Run: node --test crates/canopy-gui/ui-tests/*.test.mjs
// (scripts/test-ui.sh also syntax-checks every script first).
import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";

import { UI, loadUi, sampleOverview, scriptNames } from "./harness.mjs";

const SRC = join(UI, "..", "src");

test("every script index.html loads exists and runs", () => {
  const names = scriptNames();
  assert.ok(names.includes("app.js") && names.length >= 9, names.join(", "));
  const onDisk = readdirSync(UI).filter((f) => f.endsWith(".js"));
  assert.deepEqual([...onDisk].sort(), [...names].sort(), "a .js file in ui/ isn't loaded by index.html (or the reverse)");
  loadUi();
});

// ------------------------------------------------------------ helpers

test("esc escapes everything that could break out of HTML or an attribute", () => {
  const { run } = loadUi();
  assert.equal(run(`esc('<a href="x">\\'&')`), "&lt;a href=&quot;x&quot;&gt;&#39;&amp;");
  assert.equal(run("esc(null)"), "");
  assert.equal(run("esc(undefined)"), "");
  assert.equal(run("esc(42)"), "42");
});

test("ago rounds down to the largest unit", () => {
  const { run } = loadUi();
  const now = Math.floor(Date.now() / 1000);
  assert.equal(run("ago(0)"), "");
  assert.equal(run(`ago(${now})`), "just now");
  assert.equal(run(`ago(${now - 59})`), "just now");
  assert.equal(run(`ago(${now - 61})`), "1m ago");
  assert.equal(run(`ago(${now - 3 * 3600})`), "3h ago");
  assert.equal(run(`ago(${now - 2 * 86400})`), "2d ago");
  assert.equal(run(`ago(${now - 40 * 86400})`), "1mo ago");
  assert.equal(run(`ago(${now - 400 * 86400})`), "1y ago");
  // A clock that's behind (a commit "from the future") isn't negative.
  assert.equal(run(`ago(${now + 500})`), "just now");
});

test("plural", () => {
  const { run } = loadUi();
  assert.equal(run('plural(1, "file")'), "1 file");
  assert.equal(run('plural(0, "file")'), "0 files");
  assert.equal(run('plural(2, "line")'), "2 lines");
});

// ------------------------------------------------------------ themes

test("palette themes turn into the app's color tokens", () => {
  const { run } = loadUi();
  assert.equal(run('mix("#000000", "#ffffff", 0.5)'), "#808080");
  assert.equal(run('mix("#102030", "#102030", 0.7)'), "#102030");
  assert.ok(run('luma("#ffffff")') > 0.99 && run('luma("#000000")') === 0);
  const colors = {
    bg: "#1e1e2e", fg: "#cdd6f4", border: "#45475a", muted: "#a6adc8", accent: "#a6e3a1",
    modified: "#f9e2af", branch: "#94e2d5", hash: "#cba6f7", removed: "#f38ba8",
  };
  const look = run(`paletteVars(${JSON.stringify(colors)})`);
  assert.equal(look.dark, true);
  assert.equal(look.vars["--bg"], "#1e1e2e");
  assert.equal(look.vars["--green"], "#a6e3a1");
  // A light accent gets dark text on it; every token the CSS uses is set.
  assert.notEqual(look.vars["--on-accent"], "#ffffff");
  for (const k of run("THEME_VARS")) assert.ok(look.vars[k], `missing ${k}`);
  const light = run(`paletteVars(${JSON.stringify({ ...colors, bg: "#fafafa", accent: "#1a7f37" })})`);
  assert.equal(light.dark, false);
  assert.equal(light.vars["--on-accent"], "#ffffff");
});

test("a saved palette look is only reused for the theme it was saved for", () => {
  const { run, ctx } = loadUi();
  ctx.localStorage.setItem("canopy-theme-look", JSON.stringify({ name: "nord", dark: true, vars: {} }));
  assert.equal(run('cachedLook("nord")').name, "nord");
  assert.equal(run('cachedLook("dracula")'), null);
  ctx.localStorage.setItem("canopy-theme-look", "{not json");
  assert.equal(run('cachedLook("nord")'), null);
  assert.equal(run("savedTheme()"), "system");
});

// ------------------------------------------------------------ Home

test("sync pills say where the branch stands", () => {
  const { run } = loadUi();
  const pills = (b) => run(`syncPills(${JSON.stringify(b)})`);
  assert.match(pills({ head: null }), /detached/);
  assert.match(pills({ head: "main", upstream: null }), /not published/);
  assert.match(pills({ head: "main", upstream: "origin/main", ahead: 0, behind: 0 }), /in sync/);
  const both = pills({ head: "main", upstream: "origin/main", ahead: 2, behind: 3 });
  assert.match(both, /↑ <b>2<\/b>/);
  assert.match(both, /↓ <b>3<\/b>/);
});

test("ref chips drop HEAD, show at most two, and escape names", () => {
  const { run } = loadUi();
  const chips = (refs) => run(`refChips(${JSON.stringify(refs)})`);
  assert.equal(chips(["HEAD"]), "");
  assert.match(chips(["HEAD -> main"]), /class="ref head">main</);
  assert.match(chips(["tag: v1.0"]), /class="ref tag">v1.0</);
  assert.match(chips(["origin/main"]), /class="ref remote">origin\/main</);
  const many = chips(["HEAD -> main", "origin/main", "tag: v1", "tag: v2"]);
  assert.equal((many.match(/class="ref (head|remote|tag)"/g) || []).length, 2);
  assert.match(many, /\+2/);
  assert.match(chips(["tag: <x>"]), /&lt;x&gt;/);
});

test("Home renders an overview without undefined, NaN or raw markup", () => {
  const { run } = loadUi();
  const html = run(`renderHome(${JSON.stringify(sampleOverview())})`);
  assert.doesNotMatch(html, /undefined|NaN|\[object Object\]/);
  assert.match(html, /<h1>demo<\/h1>/);
  // The commit subject "Add <b>" is shown as text.
  assert.match(html, /Add &lt;b&gt;/);
  assert.doesNotMatch(html, /Add <b>/);
  // Next steps get their buttons.
  assert.match(html, /data-action="push"/);
  assert.match(html, /data-action="changes"/);
  // Local branches only: origin/main isn't counted.
  assert.match(html, /2 local branches/);
});

test("Home shows an operation in progress", () => {
  const { run } = loadUi();
  const html = run(`renderHome(${JSON.stringify(sampleOverview({ state: "CherryPicking" }))})`);
  assert.match(html, /Cherry-picking/);
});

test("an empty repository renders too", () => {
  const { run } = loadUi();
  const o = sampleOverview({
    log: [], branches: [], remotes: [], stashes: 0,
    status: { branch: { head: "main", oid: null, upstream: null, ahead: 0, behind: 0 }, files: [] },
    counts: { staged: 0, unstaged: 0, conflicts: 0, untracked: 0 },
    next_steps: [{ level: "info", text: "Fresh repository.", action: "connect" }],
  });
  const html = run(`renderHome(${JSON.stringify(o)})`);
  assert.doesNotMatch(html, /undefined|NaN/);
  assert.match(html, /No commits yet/);
  assert.match(html, /data-action="connect"/);
});

test("the intro card follows the seen_intro setting and the platform's undo key", () => {
  const mac = loadUi();
  mac.run("state.settings.seen_intro = false");
  assert.match(mac.run("introCard()"), /⌘Z/);
  mac.run("state.settings.seen_intro = true");
  assert.equal(mac.run("introCard()"), "");
  const win = loadUi({ platform: "Win32" });
  win.run("state.settings.seen_intro = false");
  assert.match(win.run("introCard()"), /Ctrl\+Z/);
});

test("branch rows read ahead/behind out of git's track text", () => {
  const { run } = loadUi();
  const row = (track) => run(`branchRow(${JSON.stringify({ name: "x", is_head: false, subject: "", time: 0, track })})`);
  assert.match(row("ahead 3, behind 1"), /↑ <b>3<\/b>.*↓ <b>1<\/b>/s);
  assert.match(row("gone"), /upstream gone/);
  assert.doesNotMatch(row(null), /pill/);
});

// ------------------------------------------------------------ diffs

const DIFF = [{
  old_path: "a.txt", new_path: "b.txt", binary: false,
  hunks: [{ header: "@@ -1,3 +1,3 @@", lines: [
    { kind: "Context", content: "same", old_no: 1, new_no: 1 },
    { kind: "Removed", content: "<old>", old_no: 2, new_no: null },
    { kind: "Added", content: "new", old_no: null, new_no: 2 },
    { kind: "Added", content: "", old_no: null, new_no: 3 },
    { kind: "NoNewline", content: "\\ No newline at end of file", old_no: null, new_no: null },
  ] }],
}];

test("read-only diffs count lines, show renames and escape code", () => {
  const { run } = loadUi();
  const html = run(`readonlyDiff(${JSON.stringify(DIFF)})`);
  assert.match(html, /\+2<\/span><span class="dels">−1/);
  assert.match(html, /← a\.txt/);
  assert.match(html, /&lt;old&gt;/);
  assert.doesNotMatch(html, /undefined/);
  assert.match(run("readonlyDiff([])"), /No file changes/);
  assert.match(run(`readonlyDiff([{ new_path: "x.png", binary: true, hunks: [] }])`), /Binary file/);
});

// ------------------------------------------------------------ Changes

test("Changes sorts files into conflicts, staged and unstaged", () => {
  const { run } = loadUi();
  const s = run(`sections(${JSON.stringify(sampleOverview())})`);
  assert.deepEqual(s.conflicts.map((f) => f.path), ["clash.rs"]);
  assert.deepEqual(s.staged.map((f) => f.path), ["src/lib.rs", "both.txt"]);
  // A file with staged and unstaged edits is in both lists; new files are unstaged.
  assert.deepEqual(s.unstaged.map((f) => f.path), ["README.md", "both.txt", "new.txt"]);
});

test("file badges and paths", () => {
  const { run } = loadUi();
  assert.deepEqual(run('splitPath("src/ui/app.js")'), ["src/ui/", "app.js"]);
  assert.deepEqual(run('splitPath("README.md")'), ["", "README.md"]);
  const badge = (f, sec) => run(`fileBadge(${JSON.stringify(f)}, "${sec}")`);
  assert.match(badge({ kind: "Untracked" }, "unstaged"), />\+</);
  assert.match(badge({ kind: "Tracked", index: "Deleted" }, "staged"), /red.*>D</);
  assert.match(badge({ kind: "Tracked", worktree: "Modified" }, "unstaged"), /amber.*>M</);
  assert.match(badge({ kind: "Conflicted" }, "conflicts"), />!</);
});

test("the Changes page renders its file list and banner", () => {
  const { run } = loadUi();
  const o = sampleOverview({ state: "Merging" });
  run(`state.overview = ${JSON.stringify(o)}`);
  const html = run("PAGES.changes.render(state.overview)");
  assert.doesNotMatch(html, /undefined|NaN/);
  assert.match(html, /A merge is in progress/);
  // One conflict left: Continue stays disabled.
  assert.match(html, /data-ch="continue" disabled/);
  // The first file (the conflict) is selected and shows its help.
  assert.match(html, /Keep mine \(ours\)/);
  assert.equal(run("PAGES.changes.badge(state.overview)").includes(">1<"), true);
});

test("the selection stays on the same file, or moves to the first one", () => {
  const { run } = loadUi();
  run(`state.overview = ${JSON.stringify(sampleOverview())}`);
  run('ch.sel = { path: "README.md", section: "unstaged", untracked: false }');
  assert.equal(run("fixSelection(state.overview)"), false);
  // Staged now: same path, other section, still found.
  run('ch.sel = { path: "src/lib.rs", section: "unstaged", untracked: false }');
  run("fixSelection(state.overview)");
  assert.equal(run("ch.sel.section"), "staged");
  run('ch.sel = { path: "gone.txt", section: "unstaged", untracked: false }');
  assert.equal(run("fixSelection(state.overview)"), true);
  assert.equal(run("ch.sel.path"), "clash.rs");
});

// ------------------------------------------------------------ History

test("the commit graph draws lanes and hollow merge dots", () => {
  const { run } = loadUi();
  const row = { col: 1, width: 2, up: [[0, 0], [1, 1]], down: [[1, 0]] };
  const svg = run(`graphSvg(${JSON.stringify(row)}, 2, true)`);
  assert.match(svg, /width="36"/);
  assert.match(svg, /dot hollow l1/);
  assert.equal((svg.match(/<path /g) || []).length, 3);
  assert.doesNotMatch(run(`graphSvg(${JSON.stringify(row)}, 2, false)`), /hollow/);
});

test("History reloads when a branch moves, not on every refresh", () => {
  const { run } = loadUi();
  const o = sampleOverview();
  const key = run(`refsKey(${JSON.stringify(o)})`);
  assert.equal(run(`refsKey(${JSON.stringify(sampleOverview())})`), key);
  const moved = sampleOverview();
  moved.branches[1].oid = "fffffff";
  assert.notEqual(run(`refsKey(${JSON.stringify(moved)})`), key);
  const added = sampleOverview();
  added.branches.push({ ...added.branches[1], name: "new" });
  assert.notEqual(run(`refsKey(${JSON.stringify(added)})`), key);
});

test("commit details fall back to the subject when the header is empty", () => {
  const { run } = loadUi();
  const o = sampleOverview();
  run(`state.overview = ${JSON.stringify(o)}`);
  run(`hist.data = { commits: state.overview.log, graph: [], more: false }`);
  run(`hist.sel = state.overview.log[0].oid`);
  run(`hist.details = { header: "", files: [] }`);
  assert.match(run("detailsHtml()"), /<h3 class="selectable">Add &lt;b&gt;<\/h3>/);
  run(`hist.details = { header: "commit x\\nAuthor: Ada\\n\\n    Real title\\n\\n    Body line\\n", files: [] }`);
  const html = run("detailsHtml()");
  assert.match(html, /Real title/);
  assert.match(html, /Body line/);
});

// ------------------------------------------------------------ Branches, Stash

test("Branches lists local and remote branches, leaving out origin/HEAD", () => {
  const { run } = loadUi();
  const o = JSON.stringify(sampleOverview());
  const local = run(`branchesHtml(${o})`);
  assert.match(local, /data-name="feature\/x"/);
  assert.doesNotMatch(local, /data-name="origin\/main"/);
  run('br.tab = "remote"');
  const remote = run(`branchesHtml(${o})`);
  assert.match(remote, /data-name="origin\/main"/);
  assert.doesNotMatch(remote, /origin\/HEAD/);
  run('br.tab = "tags"');
  assert.match(run(`branchesHtml(${o})`), /No tags yet/);
});

test("track pills", () => {
  const { run } = loadUi();
  assert.match(run('trackPills({ track: "gone" })'), /gone/);
  assert.equal(run("trackPills({ track: null })"), "");
  assert.match(run('trackPills({ track: "behind 4" })'), /↓ <b>4<\/b>/);
});

test("the stash list escapes messages", () => {
  const { run } = loadUi();
  run("st.loaded = true");
  run('st.list = [{ name: "stash@{0}", message: "<wip>", time: 0 }]');
  assert.match(run("stashListHtml()"), /&lt;wip&gt;/);
  run("st.list = []");
  assert.match(run("stashListHtml()"), /No stashes/);
});

// ------------------------------------------------------------ GitHub

test("label colors can't break out of the style attribute", () => {
  const { run } = loadUi();
  const html = run(`labelChips([{ name: "bug", color: "d73a4a" }, { name: "x", color: "fff;background:url(//evil)" }, { name: "y" }])`);
  assert.match(html, /--c:#d73a4a/);
  assert.doesNotMatch(html, /evil/);
  assert.equal((html.match(/--c:#888888/g) || []).length, 2);
});

test("check icons and PR state badges", () => {
  const { run } = loadUi();
  assert.match(run('checkIcon("passed")'), /green/);
  assert.match(run('checkIcon("failed")'), /red/);
  assert.equal(run('checkIcon("nope")'), '<span class="ck"></span>');
  assert.match(run('stateBadge({ state: "MERGED" })'), /violet.*merged/);
  assert.match(run('stateBadge({ state: "OPEN", isDraft: true })'), /draft/);
});

test("switching repositories forgets the last one's GitHub data", async () => {
  const { run } = loadUi({ responses: { gh_status: { state: "ready" } } });
  run(`state.overview = ${JSON.stringify(sampleOverview({ root: "/a" }))}`);
  await run("ghReady()");
  run("gh.prs.sel = 3; gh.prs.detail = { number: 3, title: 'from repo a' }; gh.failure = { run_id: 1 }; gh.homeAt = Date.now()");
  run(`state.overview = ${JSON.stringify(sampleOverview({ root: "/b" }))}`);
  await run("ghReady()");
  assert.equal(run("gh.prs.sel"), null);
  assert.equal(run("gh.prs.detail"), null);
  assert.equal(run("gh.failure"), undefined);
  assert.equal(run("gh.homeAt"), 0);
});

test("the PR list renders without undefined", () => {
  const { run } = loadUi();
  run(`gh.prs.list = [{ number: 7, title: "<Fix>", author: { login: "ada" }, headRefName: "fix", baseRefName: "main",
    updatedAt: 0, isDraft: false, reviewDecision: "APPROVED", check_state: "passed" }]`);
  const html = run("prListHtml()");
  assert.match(html, /#7/);
  assert.match(html, /&lt;Fix&gt;/);
  assert.match(html, /approved/);
  assert.doesNotMatch(html, /undefined/);
});

// ------------------------------------------------------------ the Rust side

// `invoke("name")` must name a command registered in main.rs, or the call
// fails at run time with "command not found".
test("every command the front end invokes is registered in main.rs", () => {
  const main = readFileSync(join(SRC, "main.rs"), "utf8");
  const block = main.slice(main.indexOf("generate_handler!["));
  const registered = new Set([...block.slice(0, block.indexOf("])")).matchAll(/([a-z_]+)\s*,/g)].map((m) => m[1]));
  const used = new Set();
  for (const name of scriptNames()) {
    const src = readFileSync(join(UI, name), "utf8");
    for (const m of src.matchAll(/invoke\(\s*"([a-z_]+)"/g)) used.add(m[1]);
  }
  assert.ok(used.size > 40, `only found ${used.size} invoke calls`);
  const missing = [...used].filter((c) => !registered.has(c));
  assert.deepEqual(missing, [], `invoked but not registered: ${missing.join(", ")}`);
});

// Next steps come from overview.rs; each action needs a button in the UI.
test("every next-step action from the Rust side has a button", () => {
  const src = readFileSync(join(SRC, "overview.rs"), "utf8");
  const body = src.slice(src.indexOf("pub enum StepAction"));
  const variants = [...body.slice(body.indexOf("{") + 1, body.indexOf("}")).matchAll(/^\s*([A-Z][A-Za-z]+),/gm)].map((m) => m[1].toLowerCase());
  assert.ok(variants.length >= 5, variants.join(", "));
  const { run } = loadUi();
  for (const v of variants) assert.ok(run(`ACTIONS[${JSON.stringify(v)}]?.label`), `no ACTIONS.${v} in the UI`);
});

test("every page has a title and a drawn icon, and ⌘1–9 reach them in nav order", () => {
  const { run } = loadUi();
  const ids = run("NAV.flatMap((g) => g.items)");
  assert.deepEqual(ids.slice(0, 5), ["home", "changes", "history", "branches", "stash"]);
  for (const id of ids) {
    assert.ok(run(`PAGES[${JSON.stringify(id)}].title`), id);
    assert.ok(run(`ICONS[PAGES[${JSON.stringify(id)}].icon]`), `${id} has no icon`);
  }
});

test("undo is offered after the actions the Rust side can undo", () => {
  const { run } = loadUi();
  const ops = run("UNDOABLE_OPS");
  // A hard reset's lost edits can't come back.
  assert.ok(!ops.includes("reset-hard"));
  const history = readFileSync(join(SRC, "history.rs"), "utf8");
  for (const op of ops) assert.match(history, new RegExp(`"${op}" =>`), `${op} isn't a git_op`);
});
