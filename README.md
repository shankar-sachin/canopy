# 🌳 Canopy

**A beautiful, powerful git dashboard for your terminal.**

Canopy puts your whole repository on one screen: what changed, what's staged,
where your branch stands against the remote, and what to do next. Beginners get
guidance and a safety net. Power users get line-level staging, interactive
rebase, and a command palette, all without leaving the keyboard.

<!-- Generate with: vhs demo.tape -->
![Canopy demo](demo.gif)

## Install

```sh
brew install shankar-sachin/canopy/canopy
```

Or build from source (stable Rust):

```sh
cargo install --locked --path crates/canopy
```

Canopy needs `git` on your `PATH`.

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
- [ ] Syntax-highlighted diffs
- [x] Conflict editor with per-conflict ours/theirs
- [x] File history and blame
- [x] Worktrees
- [x] Bisect
- [x] Submodules

## Develop

```sh
cargo test --workspace                          # parsers, git ops on temp repos, rendered UI flows
CANOPY_PRINT=1 cargo test -p canopy-git-tui ui_tests -- --nocapture   # print rendered frames
cargo clippy --workspace --all-targets -- -D warnings
```

Release: push a `vX.Y.Z` tag. CI builds binaries, creates the GitHub release,
and updates the Homebrew tap.

## License

MIT
