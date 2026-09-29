// A stand-in for canopy desktop's Rust side, for scripts/desktop-shots.sh.
// It answers the UI's invoke() calls from data.js (real git data from the
// dump_overview test, plus hand-written GitHub data), then sets up the scene
// named in the URL's #hash so headless Chrome can take its picture.
"use strict";

(() => {
  const D = window.MOCK_DATA;
  const scene = location.hash.slice(1) || "home";
  const hour = 3600;
  const now = Math.floor(Date.now() / 1000);
  const iso = (hoursAgo) => Math.floor(now - hoursAgo * hour);
  // The conflict scenes use a second repository, stopped mid-merge.
  const repo = scene.startsWith("conflict") ? D.merge : D.main;

  const person = (login) => ({ login });
  const check = (name, conclusion, workflowName = "CI") => ({ name, workflowName, conclusion, status: "COMPLETED", detailsUrl: "https://github.com/ada/acme-app/actions" });
  const prs = [
    { number: 14, title: "Count words case-insensitively", author: person("ada"), headRefName: "fix/case", baseRefName: "main", updatedAt: iso(1), isDraft: false, reviewDecision: "REVIEW_REQUIRED", check_state: "passed" },
    { number: 12, title: "Add top_words", author: person("grace"), headRefName: "feature/top-words", baseRefName: "main", updatedAt: iso(5), isDraft: false, reviewDecision: "APPROVED", check_state: "passed" },
    { number: 11, title: "Read from stdin when no file is given", author: person("alan"), headRefName: "stdin", baseRefName: "main", updatedAt: iso(30), isDraft: false, reviewDecision: "CHANGES_REQUESTED", check_state: "failed" },
    { number: 9, title: "Benchmark the word counter", author: person("grace"), headRefName: "bench", baseRefName: "main", updatedAt: iso(70), isDraft: true, reviewDecision: "", check_state: "pending" },
  ];
  const prDetail = {
    ...prs[1], state: "OPEN", url: "https://github.com/ada/acme-app/pull/12", additions: 8, deletions: 0, labels: [{ name: "enhancement", color: "a2eeef" }], mergeable: "MERGEABLE",
    body: "Adds top_words(counts, n), which returns the n most common words, most common first.\n\nNext: print them from main.",
    statusCheckRollup: [check("test (ubuntu-latest)", "SUCCESS"), check("test (macos-latest)", "SUCCESS"), check("test (windows-latest)", "SUCCESS"), check("clippy", "SUCCESS")],
    checks: { passed: 4, total: 4 },
    reviews: [{ author: person("ada"), state: "APPROVED", submittedAt: iso(4), body: "Nice and small. Thanks!" }],
    comments: [{ author: person("alan"), createdAt: iso(4.5), body: "Should ties be sorted alphabetically, so the output is stable?" }],
  };
  const runs = [
    { databaseId: 101, displayTitle: "Count words case-insensitively", workflowName: "CI", headBranch: "main", event: "push", createdAt: iso(0.3), number: 42, state: "failed", url: "https://github.com/ada/acme-app/actions/runs/101" },
    { databaseId: 100, displayTitle: "Ignore build output", workflowName: "CI", headBranch: "main", event: "push", createdAt: iso(51), number: 41, state: "passed", url: "https://github.com/ada/acme-app/actions/runs/100" },
    { databaseId: 99, displayTitle: "Count words in a file", workflowName: "CI", headBranch: "main", event: "push", createdAt: iso(76), number: 40, state: "passed", url: "https://github.com/ada/acme-app/actions/runs/99" },
  ];
  const step = (name, conclusion = "success") => ({ name, status: "completed", conclusion });
  const jobs = [
    { name: "test (ubuntu-latest)", status: "completed", conclusion: "failure", steps: [step("Set up job"), step("Check out"), step("Install Rust"), step("Build"), step("Run tests", "failure")] },
    { name: "clippy", status: "completed", conclusion: "success", steps: [step("Set up job"), step("Check out"), step("Clippy")] },
  ];
  const failedLog = `running 3 tests
test tests::counts_repeated_words ... ok
test tests::ignores_punctuation ... ok
test tests::empty_input ... FAILED

failures:

---- tests::empty_input stdout ----
thread 'tests::empty_input' panicked at src/main.rs:31:9:
assertion \`left == right\` failed
  left: 1
 right: 0

test result: FAILED. 2 passed; 1 failed; 0 ignored
error: test failed, to rerun pass \`--bin acme-app\``;
  const notifs = [
    { id: "n1", kind: "PullRequest", number: 14, title: "Count words case-insensitively", repo: "ada/acme-app", why: "review requested", updated_at: now - 1 * hour, unread: true, url: "https://github.com/ada/acme-app/pull/14" },
    { id: "n2", kind: "Issue", number: 7, title: "Crash on an empty file", repo: "ada/acme-app", why: "you were mentioned", updated_at: now - 3 * hour, unread: true, url: "https://github.com/ada/acme-app/issues/7" },
    { id: "n3", kind: "CheckSuite", number: null, title: "CI failed on main", repo: "ada/acme-app", why: "a CI run you started", updated_at: now - 0.3 * hour, unread: true, url: "https://github.com/ada/acme-app/actions/runs/101" },
  ];

  const settings = {
    theme: D.theme || "dark", show_commands: true, refresh_secs: 0, pull_mode: "default", check_updates: false, last_update_check: now,
    ai_assistant: "claude", editor: "", seen_intro: scene !== "learning", automation_asked: true, terminal_tip: false, last_terminal_tip: now,
  };

  const answers = {
    app_version: D.version,
    smoke_mode: false,
    initial_path: scene === "welcome" || scene === "setup" || scene === "connect" ? null : repo.overview.root,
    get_settings: settings,
    list_themes: null,
    profile: { login: "ada", name: "Ada Lovelace", git_name: "Ada Lovelace" },
    recent_repos: [
      { path: D.main.overview.root, name: "acme-app", display: "~/code/acme-app", exists: true },
      { path: "/Users/ada/code/notes", name: "notes", display: "~/code/notes", exists: true },
      { path: "/Users/ada/code/website", name: "website", display: "~/code/website", exists: true },
    ],
    open_repo: repo.overview,
    overview: repo.overview,
    refs: repo.refs,
    history: repo.history,
    last_message: repo.overview.log[0]?.subject || "",
    ask_automation: "later",
    environment: { version: D.version, terminal_app: "canopy", install: "homebrew" },
    check_update: { current: D.version, latest: D.next_version, newer: true, url: "https://github.com/shankar-sachin/canopy/releases/latest", install: "homebrew", command: "brew upgrade --cask canopy-desktop" },
    detect_editors: [],
    ai_status: { installed: ["Claude Code"], chosen: "Claude Code" },
    gh_status: { state: "ready", repo: { nameWithOwner: "ada/acme-app", defaultBranchRef: { name: "main" } } },
    gh_home: { branch_pr: null, review_requests: 1 },
    latest_failure: scene === "fix-ci" ? { run_id: 101, headline: "tests::empty_input panicked at src/main.rs:31:9", workflow: "CI", url: runs[0].url } : null,
    gh_prs: prs,
    gh_pr: prDetail,
    gh_issues: [],
    gh_runs: runs,
    gh_run_jobs: jobs,
    gh_failed_log: failedLog,
    gh_notifications: notifs,
    undo_info: { kind: "commit", last: `commit: ${repo.overview.log[0]?.subject}`, oid: repo.overview.log[1]?.oid || "", subject: repo.overview.log[1]?.subject || "", soft: true },
    inspect_folder: {
      path: "/Users/ada/code/word-count", name: "word-count", exists: true, is_repo: false, files: 3, default_branch: "main",
      gh_login: "ada", suggested: "https", has_ssh_key: true,
      https_help: "Works everywhere and signs in through your browser or a token (gh auth login sets it up). Easiest to start with.",
      ssh_help: "Uses an SSH key on this computer: no password prompts once the key is added to GitHub. Needs a key set up first.",
    },
    remote_preview: "https://github.com/ada/word-count.git",
  };

  async function invoke(cmd, args = {}) {
    if (cmd === "file_diff") return repo.diffs[`${args.path}|${args.staged}|${args.untracked}`] || [];
    if (cmd === "commit_details") return repo.all_details[args.rev] || repo.details;
    if (cmd === "stash_diff") return repo.stash_diffs[args.name] || [];
    if (cmd in answers) return answers[cmd];
    // Anything that would change something does nothing here.
    return null;
  }
  window.__TAURI__ = { core: { invoke }, event: { listen: async () => () => {} } };

  // ------------------------------------------------------------ scenes

  const wait = (ms) => new Promise((r) => setTimeout(r, ms));
  async function until(sel) {
    for (let i = 0; i < 100 && !document.querySelector(sel); i++) await wait(50);
    return document.querySelector(sel);
  }
  const click = async (sel) => (await until(sel))?.dispatchEvent(new MouseEvent("click", { bubbles: true }));

  const SCENES = {
    home: async () => {},
    learning: async () => {
      document.querySelector('[data-what="Staged"]')?.click();
    },
    changes: async () => {
      go("changes");
      await until(".dl.pick");
      // Pick three changed lines, as if clicked.
      // Each click redraws the diff, so look the lines up again every time.
      for (const i of [1, 2, 3]) {
        document.querySelectorAll(".dl.pick")[i]?.dispatchEvent(new MouseEvent("click", { bubbles: true }));
        await wait(50);
      }
      $("#c-summary").value = "Ignore case and punctuation when counting";
    },
    amend: async () => {
      go("changes");
      await until("#c-amend");
      $("#c-amend").checked = true;
      $("#c-amend").dispatchEvent(new Event("change"));
      await wait(100);
    },
    history: async () => {
      go("history");
      await until(".cdetails .dfile");
    },
    reset: async () => {
      go("history");
      await until(".cdetails .dfile");
      const rows = document.querySelectorAll(".hrow");
      rows[1]?.dispatchEvent(new MouseEvent("click", { bubbles: true }));
      await until('[data-h="reset"]');
      await wait(100);
      await click('[data-h="reset"]');
    },
    branches: async () => {
      go("branches");
      await until(".brow");
    },
    tags: async () => {
      go("branches");
      await until(".brow");
      for (let i = 0; i < 100 && !br.tags.length; i++) await wait(50);
      br.tab = "tags";
      $("#branches").innerHTML = branchesHtml(state.overview);
    },
    stash: async () => {
      go("stash");
      await until(".srow");
      await until("#sdiff .dfile");
    },
    conflict: async () => {
      go("changes");
      await until(".conflict-help");
    },
    undo: async () => {
      undo();
      await until(".modal");
    },
    diverged: async () => {
      state.overview.status.branch.ahead = 1;
      state.overview.status.branch.behind = 2;
      render();
      sync("push");
      await until(".modal");
    },
    welcome: async () => {},
    setup: async () => {
      await until("#welcome:not([hidden])");
      await startSetup("/Users/ada/code/word-count");
      su.step = "init";
      drawSetup();
    },
    connect: async () => {
      await until("#welcome:not([hidden])");
      await startSetup("/Users/ada/code/word-count");
      su.step = "remote";
      su.remote = "existing";
      drawSetup();
      await wait(100);
      const input = $("#su-remote");
      if (input) { input.value = "ada/word-count"; input.dispatchEvent(new Event("input", { bubbles: true })); }
    },
    prs: async () => {
      go("prs");
      await until(".gdetail .checks");
    },
    review: async () => {
      go("prs");
      await until(".gdetail .checks");
      await click('[data-g="pr-review"]');
    },
    runs: async () => {
      go("runs");
      await until(".gdetail .log");
    },
    "fix-ci": async () => {
      await until(".fail-banner");
    },
    settings: async () => {
      setts.update = await invoke("check_update");
      await openSettings("updates");
    },
    "settings-general": async () => {
      await openSettings("general");
    },
    notifications: async () => {
      go("notifs");
      await until(".gdetail");
    },
  };

  window.addEventListener("DOMContentLoaded", async () => {
    // start() in app.js runs first (it was added first); wait for it.
    if (scene !== "welcome" && scene !== "setup" && scene !== "connect") await until("#shell:not([hidden]) .hero");
    await wait(200);
    await (SCENES[scene] || SCENES.home)();
    await wait(300);
    document.body.dataset.ready = scene;
  });
})();
