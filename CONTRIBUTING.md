# Contributing to canopy

Thanks for helping. There are two apps here, canopy desktop (`canopy-desktop`)
and canopy console (`canopy`), plus the website and wiki in `docs/`.
Fixes, features, docs and wiki edits are all welcome.

## The one rule: everything goes through a pull request

**Want to add or change something? Open a pull request.** Nobody pushes
straight to `main`, and that includes the maintainer. Every change, even a
one-word typo fix, gets its own branch and a PR so CI can check it and
someone can review it.

1. Fork the repository (or make a branch, if you have write access).
2. Make a branch from an up-to-date `main`:
   `git switch -c fix/login-crash main`
3. Commit your work there, push the branch, and open a PR against `main`.
4. Wait for CI to go green and for a review. Push more commits to the same
   branch to answer comments; don't open a new PR for each round.
5. The maintainer merges it. Please don't merge your own PR.

Also:

- **Don't force-push to `main`** or to anyone else's branch. Force-pushing
  your own PR branch (after a rebase, say) is fine.
- **Don't rewrite published history** on `main`: no rebasing it, no
  amending merged commits, no deleting tags.
- **One change per PR.** A bug fix and an unrelated refactor are two PRs.
  Small PRs get reviewed faster.
- **Bigger ideas start as an issue.** For a new feature, a new page on the
  site or anything that changes how the apps work, open an issue first so
  we can agree on the approach before you spend time on it.
- **Security problems don't go in issues or PRs.** Report them privately;
  see [SECURITY.md](SECURITY.md).

## Before you open the PR

Run the same checks CI runs:

```sh
scripts/check.sh      # cargo fmt, clippy (warnings are errors), every Rust test
scripts/audit.sh      # all of the above, plus the desktop UI tests, the website audit and more
```

`scripts/audit.sh --quick` skips the Rust step if you only touched the
desktop UI or the website. CI runs on macOS, Linux and Windows, so avoid
anything that only works on one of them (paths, shells, line endings).

Other things a PR should have:

- **Tests** for new behavior and for the bug you fixed. Rust tests live next
  to the code; desktop UI tests are in `crates/canopy-gui/ui-tests` and run
  with `scripts/test-ui.sh` (plain `node --test`, no npm).
- **Docs** when something visible changes: the wiki page in `docs/wiki/`,
  and a line in `docs/changelog.html` under the next version.
- **Fresh screenshots** if canopy console's screens changed:
  `scripts/screens.sh` regenerates the site's screenshots and key tables.
- **No new dependencies without asking.** canopy keeps its dependency list
  short on purpose. If you need a new crate, say why in an issue or the PR
  description first. The desktop front end is plain HTML, CSS and JS with no
  npm and no build step, and it stays that way.

## Style

- Rust: `cargo fmt` (see `rustfmt.toml`) and clippy with `-D warnings`.
- Match the code around you: naming, comment density, how errors are shown.
- Words in the apps and on the site are plain and short, written for someone
  new to git. Explain what will happen before anything destructive, and say
  how to undo it.
- Only single-width characters are drawn in canopy console (a test checks this).
- Commit messages: a short summary line that says what changed, then a blank
  line and the why, if it isn't obvious.

## Releases

Only the maintainer tags and releases, with `scripts/release.sh`. Versions
are patch bumps (1.0.13, 1.0.14, ...). Please don't change the version in
`Cargo.toml` in your PR.

## License

By contributing, you agree that your work is released under the
[MIT License](LICENSE), like the rest of canopy.
