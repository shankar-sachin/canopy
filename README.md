<p align="center"><img src="docs/assets/logo.svg" alt="Canopy logo" width="96" height="96"></p>

<h1 align="center">Canopy</h1>

**A beautiful, powerful git dashboard for your terminal.**

Canopy puts your whole repository on one screen: what changed, what's staged,
where your branch stands against the remote, and what to do next. Beginners get
guidance and a safety net. Power users get line-level staging, interactive
rebase, and a command palette, all without leaving the keyboard.

**Website:** [shankar-sachin.github.io/canopy](https://shankar-sachin.github.io/canopy/), with a feature tour, the GitHub guide, and the full key reference.

<!-- Generate with: vhs demo.tape -->
![Canopy demo](demo.gif)

## Install

```sh
brew install shankar-sachin/canopy/canopy
```

Or, without Homebrew (macOS and Linux x86_64; installs to `~/.local/bin` and verifies the checksum):

```sh
curl -fsSL https://raw.githubusercontent.com/shankar-sachin/canopy/main/scripts/install.sh | sh
```

Or build from source (stable Rust):

```sh
cargo install --locked --path crates/canopy
```

Canopy needs `git` on your `PATH`.

**On Windows**, in PowerShell:

```powershell
irm https://raw.githubusercontent.com/shankar-sachin/canopy/main/scripts/install.ps1 | iex
```

or download `canopy-<version>-x86_64-pc-windows-msvc.zip` from
[Releases](https://github.com/shankar-sachin/canopy/releases) and put
`canopy.exe` on your `PATH`.
Windows Terminal is recommended. Full steps are on the
[website](https://shankar-sachin.github.io/canopy/get-started.html#windows).

## Use

```sh
canopy                 # open the repo you're in
canopy ~/code/project  # open a specific repo
canopy -w ~/code       # workspace view: every repo under ~/code
```

| Tab | What it shows |
| --- | --- |
| **1 Home** | Branch and sync status, "next steps" suggestions, recent commits with a graph, activity, branches |
| **2 Changes** | Staged / unstaged / conflicted files with a live diff. Stage files, hunks, or single lines |
| **3 History** | Commit graph with search. Checkout, cherry-pick, revert, reset, tag, fixup, interactive rebase |
| **4 Branches** | Branches, tags, remotes, worktrees, and submodules (`[` / `]` to switch). Checkout, create, rename, delete, merge, rebase; tag and push tags; add and edit remotes; open a branch in its own worktree folder |
| **5 Stash** | Apply, pop, drop, and preview stashes |
| **6 Workspace** | Every repo in your workspace dirs: branch, dirty state, sync. Open any, or fetch them all |
| **7 Reflog** | Everywhere HEAD has been: your safety net for recovering anything |
| **8 Pull requests** | GitHub PRs (open / mine / review requested / all) with checks and reviews. Check out, create, review, comment, merge, close, open in the browser. Needs the [GitHub CLI](https://cli.github.com) logged in |
| **9 Issues** | GitHub issues (open / assigned to me / all) with labels and comments. Create, comment, close / reopen, open in the browser |
| **0 Actions** | GitHub Actions runs for this branch or all branches, updating live while they run. Jobs, failing steps, and the tail of the failed log; re-run failed jobs |

### Keys

Press `?` on any screen for its keys, or `:` to search every action by name.

| Key | Action | | Key | Action |
| --- | --- | --- | --- | --- |
| `1`–`9`, `0`, `tab` | switch tabs | | `c` | commit |
| `j`/`k`, `↑`/`↓` | move | | `A` | amend last commit |
| `space` | stage / unstage | | `P` / `p` / `f` | push / pull / fetch |
| `⏎` | open diff (then `space` = line, `⏎` = hunk, `v` = range) | | `S` | stash changes |
| `a` | stage / unstage all | | `z` | **undo** last commit, reset, or checkout |
| `d` | discard (asks first) | | `!` | run any git command |
| `/` | filter the list | | `T` | toggle teach mode |
| `L` | history of a file (follows renames) | | `B` | blame: who changed each line |
| `b` | bisect: find the commit that broke something | | | |
| `:` or `ctrl-p` | command palette | | `ctrl-t` | cycle theme |
| `⌃Q` / `⌥Q` | quit, from anywhere (even in a dialog) | | `⌥1`…`⌥0` | jump to a tab |
| `⌥←` / `⌥→` | previous / next tab | | `⌥C` `⌥P` `⌥⇧P` | commit, pull, push |

⌃ is Control and ⌥ is Option (Alt on other keyboards). On macOS Canopy shows
keys this way; on Linux it shows `ctrl-q` / `alt-q` (set `mac_key_symbols` to choose).

### Made for learning git

- **Teach mode** (on by default): the footer shows the exact `git` command
  behind every action, so you pick up git as you go. Toggle it with `T`.
- **Next steps** on Home tell you, in plain language, what state you're in and
  which key moves you forward: staged files to commit, commits to push, a merge
  in progress.
- **Confirmations** explain what a destructive action will throw away.
- **Undo** (`z`) reverses the last commit (your changes come back staged), reset,
  merge, or checkout, using the reflog.

## Configure

`~/.config/canopy/config.toml` (run `canopy --example-config` for a template):

```toml
theme = "canopy"            # canopy | catppuccin | gruvbox | nord | light
nerd_font = false
teach_mode = true
confirm_destructive = true
compact = false             # true: tighter layout, no gaps between panels
splash = true               # the tree animation when Canopy starts (any key skips)
workspace_dirs = ["~/code"]
workspace_depth = 3

# Remap any action by its snake_case name (see the full list with `?`).
# A remapped key is taken away from whatever used it before, and Canopy
# warns you if that leaves an action without a key.
[keys]
toggle_stage = "s"
commit = ["c", "ctrl-s"]

[[custom_commands]]
key = "X"
cmd = "git push --force-with-lease"
description = "force push (safely)"
confirm = true
```

## How it works

Canopy runs the real `git` binary and parses its machine-readable output
(`status --porcelain=v2 -z`, `for-each-ref`, custom `log` formats). Your hooks,
config, credential helpers, and signing all behave exactly as they do on the
command line. Line staging builds a minimal patch and applies it to the index
with `git apply --cached`.

```
crates/canopy-git   git runner, parsers, typed operations (no UI)
crates/canopy       the TUI (Ratatui): app loop, screens, keymap, themes
```

## Roadmap

- [x] Full git dashboard (v1)
- [x] GitHub pull requests and issues via `gh`
- [x] GitHub Actions runs
- [x] The Canopy wiki, the logo, a startup animation, and a roomier layout (v0.4)
- [x] More GitHub: notifications, releases, inline review comments, a CI log viewer, and Windows builds (v1.0)
- [ ] A desktop GUI
- [ ] Syntax-highlighted diffs
- [x] Conflict editor with per-conflict ours/theirs
- [x] File history and blame
- [x] Worktrees
- [x] Bisect
- [x] Submodules

## Website

The site lives in `docs/` and is served by GitHub Pages from `main`. Its
screenshots and key reference are rendered from the real app:

```sh
cargo test -p canopy-git-tui export_site_screens -- --ignored
```

## Develop

```sh
scripts/check.sh      # fmt + clippy + every test, with a one-line verdict (what CI runs)
scripts/screens.sh    # regenerate the website's screenshots and key tables
CANOPY_PRINT=1 cargo test -p canopy-git-tui ui_tests -- --nocapture   # print rendered frames
```

| Script | What it does |
| --- | --- |
| `scripts/install.sh` | Install a release on macOS/Linux (`… \| sh -s -- v1.0.0` for a version, `--uninstall` to remove) |
| `scripts/install.ps1` | The same for Windows (`-Version`, `-Uninstall`) |
| `scripts/check.sh` | Formatting, clippy with warnings as errors, and all tests |
| `scripts/screens.sh` | Re-render the docs screenshots, keys and theme tables from the real TUI |
| `scripts/release.sh` | Tag the version in `Cargo.toml`, publish the release as you, wait for the builds, and check the Homebrew tap |

Release: bump `version` in `Cargo.toml` in a PR, merge it, then run
`scripts/release.sh` on main. CI builds the binaries, attaches them to the
release, and updates the Homebrew tap.

## License

MIT
