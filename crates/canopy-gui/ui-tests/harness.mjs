// Loads the desktop app's front end (ui/*.js, in index.html's order) into a
// sandbox with just enough of a browser to run it: no npm, no real DOM.
// Pure helpers (esc, ago, syncPills, readonlyDiff, ...) and page renderers
// can then be called and checked. `invoke` calls are recorded and answered
// from `responses`.
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import vm from "node:vm";

export const UI = join(dirname(fileURLToPath(import.meta.url)), "..", "ui");

/// The scripts index.html loads, in order.
export function scriptNames() {
  const html = readFileSync(join(UI, "index.html"), "utf8");
  return [...html.matchAll(/<script src="([^"]+)"><\/script>/g)].map((m) => m[1]);
}

// An element that accepts whatever the scripts do to it at load time.
function fakeEl(tag = "div") {
  const kids = [];
  return {
    tagName: tag.toUpperCase(),
    dataset: {},
    style: { setProperty() {}, removeProperty() {}, getPropertyValue: () => "" },
    classList: { add() {}, remove() {}, toggle() {}, contains: () => false },
    attributes: {},
    hidden: false,
    textContent: "",
    innerHTML: "",
    value: "",
    title: "",
    children: kids,
    addEventListener() {},
    removeEventListener() {},
    append(...c) { kids.push(...c); },
    remove() {},
    focus() {},
    querySelector: () => null,
    querySelectorAll: () => [],
    closest: () => null,
    replaceWith() {},
    cloneNode() { return fakeEl(tag); },
    scrollIntoView() {},
  };
}

/// A fresh sandbox with every UI script loaded. `platform` is what
/// navigator.platform says ("MacIntel", "Win32", "Linux x86_64").
// What the Rust side answers when a test doesn't say: GitHub isn't set up.
const DEFAULT_RESPONSES = { gh_status: { state: "not_installed" }, ai_status: { installed: [], chosen: null } };

export function loadUi({ platform = "MacIntel", responses = {} } = {}) {
  responses = { ...DEFAULT_RESPONSES, ...responses };
  const calls = [];
  const store = new Map();
  const byId = new Map();
  const document = {
    title: "",
    documentElement: fakeEl("html"),
    body: fakeEl("body"),
    visibilityState: "visible",
    // `$("#id")` hands back the same element every time, so code that
    // writes to one can be checked.
    querySelector(sel) {
      if (!byId.has(sel)) byId.set(sel, fakeEl());
      return byId.get(sel);
    },
    querySelectorAll: () => [],
    createElement: (tag) => fakeEl(tag),
    addEventListener() {},
    removeEventListener() {},
  };
  const invoke = async (cmd, args) => {
    calls.push({ cmd, args });
    const r = responses[cmd];
    if (r instanceof Error) throw r;
    return typeof r === "function" ? r(args) : r;
  };
  const ctx = {
    console,
    document,
    navigator: { platform, clipboard: { writeText: async () => {} } },
    window: { addEventListener() {}, __TAURI__: { core: { invoke }, event: { listen() {} } } },
    localStorage: {
      getItem: (k) => (store.has(k) ? store.get(k) : null),
      setItem: (k, v) => store.set(k, String(v)),
      removeItem: (k) => store.delete(k),
    },
    getComputedStyle: () => ({ getPropertyValue: () => "" }),
    // Timers never fire: nothing in a test should wait on a toast.
    setTimeout: () => 0,
    clearTimeout() {},
    setInterval: () => 0,
    clearInterval() {},
    MouseEvent: class {},
  };
  ctx.window.document = document;
  vm.createContext(ctx);
  for (const name of scriptNames()) {
    vm.runInContext(readFileSync(join(UI, name), "utf8"), ctx, { filename: name });
  }
  // Top-level `const`s live in the script scope, not on the global object,
  // so reach them through an expression evaluated inside the sandbox.
  // Objects come back through JSON, so they compare with this realm's
  // deepEqual (the sandbox has its own Array and Object); promises don't.
  const run = (expr) => {
    const v = vm.runInContext(expr, ctx);
    if (v && typeof v === "object" && typeof v.then !== "function") return JSON.parse(JSON.stringify(v));
    return v;
  };
  return { ctx, run, calls, el: (sel) => document.querySelector(sel) };
}

/// A small repository overview, shaped like the Rust side's `Overview`.
export function sampleOverview(over = {}) {
  const now = Math.floor(Date.now() / 1000);
  return {
    name: "demo",
    root: "/tmp/demo",
    state: "Clean",
    status: {
      branch: { head: "main", oid: "a1b2c3d4e5f60718293a4b5c6d7e8f9012345678", upstream: "origin/main", ahead: 1, behind: 0 },
      files: [
        { path: "src/lib.rs", kind: "Tracked", index: "Modified", worktree: "Unmodified", orig_path: null },
        { path: "README.md", kind: "Tracked", index: "Unmodified", worktree: "Modified", orig_path: null },
        { path: "both.txt", kind: "Tracked", index: "Added", worktree: "Modified", orig_path: null },
        { path: "new.txt", kind: "Untracked", index: "Unmodified", worktree: "Unmodified", orig_path: null },
        { path: "clash.rs", kind: "Conflicted", index: "Unmodified", worktree: "Unmodified", orig_path: null },
      ],
    },
    counts: { staged: 2, unstaged: 3, conflicts: 1, untracked: 1 },
    stashes: 1,
    branches: [
      { name: "main", is_remote: false, is_head: true, oid: "a1b2c3d", upstream: "origin/main", track: "ahead 1", subject: "Add <b>", time: now - 60 },
      { name: "feature/x", is_remote: false, is_head: false, oid: "b2c3d4e", upstream: null, track: null, subject: "WIP", time: now - 7200 },
      { name: "origin/main", is_remote: true, is_head: false, oid: "c3d4e5f", upstream: null, track: null, subject: "Base", time: now - 86400 },
      { name: "origin/HEAD", is_remote: true, is_head: false, oid: "c3d4e5f", upstream: null, track: null, subject: "Base", time: now - 86400 },
    ],
    remotes: [{ name: "origin", fetch_url: "https://github.com/ada/demo.git" }],
    log: [
      { oid: "a1b2c3d4e5f60718293a4b5c6d7e8f9012345678", short: "a1b2c3d", subject: "Add <b>", author: "Ada", email: "ada@example.com", time: now - 60, parents: ["c3d4e5f"], refs: ["HEAD -> main", "tag: v1.0.0"] },
      { oid: "c3d4e5f60718293a4b5c6d7e8f901234567890ab", short: "c3d4e5f", subject: "Base", author: "Ada", email: "ada@example.com", time: now - 86400, parents: [], refs: ["origin/main", "origin/HEAD"] },
    ],
    next_steps: [
      { level: "info", text: "You have 1 commit to push.", action: "push" },
      { level: "warn", text: "Resolve 1 conflict.", action: "changes" },
    ],
    ...over,
  };
}
