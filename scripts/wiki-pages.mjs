// The wiki's page list, shared by scripts/wiki.mjs (which builds the pages)
// and scripts/audit-site.mjs (which checks them).
//
// The wiki has a home, a few pages for both apps, and two guides, one for
// canopy desktop (docs/wiki/desktop/) and one for canopy console (docs/wiki/console/).
// Previous/Next follow each guide's order.

// [file, title, lede, card]. `lede` is the line under the title (and the meta
// description); `card` is the text on the guide's home page.
const SHARED = [
  ["index.html", "The canopy wiki", "Learn git with canopy: one guide for canopy desktop, one for canopy console, and the ideas both share."],
  ["git-basics.html", "Git in ten minutes", "The handful of ideas that make git make sense, and where to see each one in canopy.", "Repositories, the staging area, commits, branches and remotes, each shown where it lives in canopy."],
  ["themes.html", "Themes", "The built-in themes, how to switch in each app, and how to make your own.", "The built-in themes, and making your own."],
];

// Pages both guides have, worded per app in the guide.
const desktopGuides = [
  ["daily-workflow.html", "Your daily workflow", "The loop you'll repeat every day in canopy desktop: pull, branch, edit, stage, commit, push, open a pull request.", "Pull, branch, edit, stage, commit, push, open a PR: the loop you'll do every day."],
  ["commits.html", "Write good commits", "Small, focused commits with clear messages, and how to stage exactly the lines that belong together.", "Small commits, clear messages, and staging just the lines that belong together."],
  ["branches.html", "Branches, merging and rebasing", "What branches are for, the two ways to combine them, and what to do when they collide.", "Why branches, how merging and rebasing differ, and what to do when they collide."],
  ["syncing.html", "Sync with a remote", "Fetch, pull and push, what \"ahead\" and \"behind\" mean, and what to do when your branch and the remote have both moved on.", "Fetch, pull and push; ahead and behind; a diverged branch; force-with-lease."],
  ["github-flow.html", "Working with GitHub", "Connect canopy desktop to GitHub, then handle pull requests, CI runs and notifications without opening a browser.", "Connect GitHub, then pull requests, CI and notifications."],
];
const consoleGuides = desktopGuides.map(([f, t, l, c]) => [f, t, l.replace("canopy desktop", "canopy console"), c]);

const desktopRecipes = [
  ["fix-last-commit.html", "Fix your last commit", "A typo in the message, a forgotten file, or a fix that belongs in an older commit.", "Typo in the message, a forgotten file, or a fix for an older commit."],
  ["undo.html", "Undo a mistake", "Take back a commit, a reset, a merge or a checkout, and get back work you thought was lost.", "Undo a commit, a reset, a merge or a checkout, and get back \"lost\" work."],
  ["conflicts.html", "Resolve a merge conflict", "When a merge, rebase or cherry-pick stops because both sides changed the same lines.", "Pick a side (or both) for each conflict, then finish the merge or rebase."],
  ["stash.html", "Put work aside with the stash", "Save half-done changes without committing them, switch to something else, and bring them back later.", "Save half-done work, switch tasks, and bring it back later."],
  ["ignore-files.html", "Ignore files with .gitignore", "Keep build output, secrets and editor clutter out of git, and stop tracking a file you committed by mistake.", "Keep build output, secrets and clutter out of git."],
  ["tags-releases.html", "Tag a version and publish a release", "Mark a commit as v1.2.0, push the tag, and publish a GitHub release with notes.", "Tag a commit, push the tag, and publish a GitHub release."],
  ["review-a-pr.html", "Review a pull request", "Read a pull request, comment, and approve or ask for changes.", "Read the diff, comment, then approve or request changes."],
  ["fix-ci.html", "Fix a failing CI run", "Find out why CI failed, fix it (or re-run it), and get back to green.", "Find why CI failed, fix it, push, and re-run."],
  ["new-repo.html", "Start a new repository", "Turn a folder into a git repository, then put it on GitHub over HTTPS or SSH.", "Set up a folder, then connect GitHub over HTTPS or SSH."],
];
const consoleRecipes = [
  ...desktopRecipes.slice(0, 5),
  ["clean-history.html", "Tidy up commits before sharing", "Squash, reorder, reword and drop commits with an interactive rebase.", "Squash, reorder, reword or drop commits with a visual interactive rebase."],
  ["find-a-bug.html", "Find the commit that broke something", "Bisect to the commit that introduced a bug, then use blame and file history to understand it.", "Bisect to the breaking commit, then blame and file history to understand it."],
  ["two-branches.html", "Work on two branches at once", "Check out a second branch in its own folder with a worktree, so you never have to stash to switch.", "Check out a second branch in its own folder with worktrees."],
  ...desktopRecipes.slice(5),
];

export const GUIDES = [
  {
    id: "desktop",
    name: "canopy desktop",
    short: "Desktop",
    dir: "desktop",
    blurb: "The friendly app for macOS, Windows and Linux. Start here if you're new to git.",
    home: ["index.html", "The canopy desktop guide", "Everything in canopy desktop, page by page: guides for the daily loop, recipes for everyday fixes, and reference for shortcuts and settings."],
    groups: [
      ["Start here", [["tour.html", "A tour of canopy desktop", "Every page of the desktop app, what it shows, and what you can do there.", "Home, Changes, History, Branches, Stash and the GitHub pages, one at a time."]]],
      ["Guides", desktopGuides],
      ["Recipes", desktopRecipes],
      ["Reference", [
        ["keys.html", "Keyboard shortcuts", "Every keyboard shortcut in canopy desktop, on macOS, Windows and Linux.", "Every shortcut in canopy desktop."],
        ["settings.html", "Settings", "Every setting in canopy desktop's Settings page, what it does, and its default.", "Themes, refresh, pull mode, updates, and the AI assistant."],
        ["highlighting.html", "Syntax highlighting and live refresh", "How canopy desktop colors code in diffs and notices changes as you make them.", "Colored diffs and instant updates."],
        ["troubleshooting.html", "Troubleshooting", "Fixes for the problems people run into with canopy desktop.", "Fixes for the problems people actually hit."],
        ["faq.html", "FAQ", "Short answers to common questions about canopy desktop.", "Short answers to common questions."],
      ]],
    ],
  },
  {
    id: "console",
    name: "canopy console",
    short: "Console",
    dir: "console",
    blurb: "The keyboard-driven power tool that runs in your terminal, with everything canopy can do.",
    home: ["index.html", "The canopy console guide", "Everything in canopy console, page by page: guides for the daily loop, recipes for everyday fixes and power tools, and reference for keys and config."],
    groups: [
      ["Start here", [["tour.html", "A tour of canopy console", "Every screen of canopy console, what it shows, and the key to get there.", "Home, Changes, Log, Refs, Stash, Repos, Reflog, PRs, Issues and CI, one at a time."]]],
      ["Guides", consoleGuides],
      ["Recipes", consoleRecipes],
      ["Reference", [
        ["keys.html", "Keyboard shortcuts", "Every key in canopy console, by screen, generated from the app.", "Every key, by screen."],
        ["config.html", "Configuration", "Every setting in canopy console's config.toml, with its default.", "Every setting in config.toml, with its default."],
        ["cli.html", "Command line", "The canopy command's flags, arguments and environment variables.", "Flags, arguments and environment variables."],
        ["highlighting.html", "Syntax highlighting and live refresh", "How canopy console colors code in diffs and notices changes as you make them, and how to turn either off.", "Colored diffs and instant updates, and the settings."],
        ["troubleshooting.html", "Troubleshooting", "Fixes for the problems people run into with canopy console.", "Fixes for the problems people actually hit."],
        ["faq.html", "FAQ", "Short answers to common questions about canopy console.", "Short answers to common questions."],
      ]],
    ],
  },
];

// Pages in a guide that follow the five-section recipe shape.
export const RECIPE_SECTIONS = ["steps", "git", "wrong", "related"];
export const SHAPED_GROUPS = ["Recipes"];
export const SHAPED_FILES = ["daily-workflow.html", "commits.html", "branches.html", "syncing.html", "github-flow.html"];

// Every page: { file (relative to docs/wiki), title, lede, card, group, guide (id or null), name (of the guide), dir }.
export const PAGES = [
  ...SHARED.map(([file, title, lede, card]) => ({ file, title, lede, card: card || lede, group: "Wiki", guide: null, name: "canopy wiki" })),
  ...GUIDES.flatMap((g) => [
    { file: `${g.dir}/${g.home[0]}`, title: g.home[1], lede: g.home[2], card: g.blurb, group: "Home", guide: g.id, name: g.name },
    ...g.groups.flatMap(([group, pages]) => pages.map(([f, title, lede, card]) => ({ file: `${g.dir}/${f}`, title, lede, card: card || lede, group, guide: g.id, name: g.name }))),
  ]),
];

// The reading order inside a guide: its home, then every page in turn.
export const guidePages = (id) => PAGES.filter((p) => p.guide === id);

// Pages that used to live at docs/wiki/<file> before the split into two
// guides. They become small "this page moved" pages.
export const LEGACY = {
  "daily-workflow.html": ["desktop", "console"],
  "commits.html": ["desktop", "console"],
  "branches.html": ["desktop", "console"],
  "syncing.html": ["desktop", "console"],
  "github-flow.html": ["desktop", "console"],
  "fix-last-commit.html": ["desktop", "console"],
  "undo.html": ["desktop", "console"],
  "conflicts.html": ["desktop", "console"],
  "stash.html": ["desktop", "console"],
  "ignore-files.html": ["desktop", "console"],
  "tags-releases.html": ["desktop", "console"],
  "review-a-pr.html": ["desktop", "console"],
  "fix-ci.html": ["desktop", "console"],
  "new-repo.html": ["desktop", "console"],
  "highlighting.html": ["desktop", "console"],
  "troubleshooting.html": ["desktop", "console"],
  "faq.html": ["desktop", "console"],
  "clean-history.html": ["console"],
  "find-a-bug.html": ["console"],
  "two-branches.html": ["console"],
  "config.html": ["console"],
  "cli.html": ["console"],
  "desktop-tour.html": ["desktop"],
  "desktop-keys.html": ["desktop"],
};

// Old names that changed as well as moved.
export const RENAMED = { "desktop-keys.html": "keys.html", "desktop-tour.html": "tour.html" };
