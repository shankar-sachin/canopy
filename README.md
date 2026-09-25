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
| **4 Branches** | Local and remote branches with ahead/behind. Checkout, create, rename, delete, merge, rebase |
| **5 Stash** | Apply, pop, drop, and preview stashes |
| **6 Workspace** | Every repo in your workspace dirs: branch, dirty state, sync. Open any, or fetch them all |
| **7 Reflog** | Everywhere HEAD has been: your safety net for recovering anything |

### Keys

Press `?` on any screen for its keys, or `:` to search every action by name.

| Key | Action | | Key | Action |
| --- | --- | --- | --- | --- |
| `1`–`7`, `tab` | switch tabs | | `c` | commit |
| `j`/`k`, `↑`/`↓` | move | | `A` | amend last commit |
| `space` | stage / unstage | | `P` / `p` / `f` | push / pull / fetch |
| `⏎` | open diff (then `space` = line, `⏎` = hunk, `v` = range) | | `S` | stash changes |
| `a` | stage / unstage all | | `z` | **undo** last commit, reset, or checkout |
| `d` | discard (asks first) | | `!` | run any git command |
| `/` | filter the list | | `T` | toggle teach mode |
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
- [ ] GitHub via `gh`: pull requests, issues, Actions runs, reviews
- [ ] Syntax-highlighted diffs
- [ ] Conflict editor with per-hunk ours/theirs
- [ ] Worktrees, submodules, blame, bisect screens

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
