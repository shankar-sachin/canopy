# Security policy

## Supported versions

canopy ships patch releases often, and fixes land in the newest one. Only the
latest release gets security fixes, so please update before reporting:

| Version | Supported |
| --- | --- |
| Latest release ([releases](https://github.com/shankar-sachin/canopy/releases/latest)) | Yes |
| Anything older | No: update to the latest |

This covers everything in this repository: canopy console (`canopy`),
canopy desktop (`canopy-desktop`), the install scripts in `scripts/`, the
Homebrew formula and cask, the winget manifests, and the website in `docs/`.

## Reporting a vulnerability

**Please don't open a public issue for a security problem.** Report it
privately instead:

1. Go to the repository's [Security tab](https://github.com/shankar-sachin/canopy/security).
2. Click **Report a vulnerability** and fill in the form.

Only the maintainer sees the report. Please include:

- which app and version (`canopy --version`, or Settings → About in the desktop app),
  and your operating system;
- what an attacker can do, and what they need first (a crafted repository, a
  malicious branch name, a link, access to your machine...);
- steps to reproduce, ideally with a small example repository.

## What to expect

- A reply within **7 days** confirming the report.
- A fix in a patch release as soon as it's ready. Serious issues come first.
- Credit in the changelog and the GitHub advisory if you'd like it. Tell us
  the name to use, or say you'd rather stay anonymous.

Please give us a reasonable time to ship a fix before you disclose the problem
publicly. We'll agree on a date together.

## How canopy keeps you safe

Useful context when judging whether something is a vulnerability:

- **canopy runs the real `git` and `gh`** on your machine, with your own login.
  It stores no tokens or passwords; GitHub credentials stay with the GitHub CLI.
- **Nothing is sent anywhere by canopy itself** except what git and gh do for
  you. The desktop app also checks GitHub for new releases once a day (turn it
  off in Settings → Updates); canopy console doesn't.
- **Names you type are never read as options.** A branch, tag, remote or stash
  name that starts with `-` is refused before it reaches git. Paths are passed
  after `--`.
- **Destructive actions ask first** (discard, hard reset, force push, delete),
  and Undo can take back the last commit, reset or checkout.
- **The desktop app's pages escape everything they show** (commit messages,
  file names, PR titles, labels) and run under a strict Content Security Policy
  with no remote scripts.
- **Custom commands** in `config.toml` run exactly what you wrote there, as
  you. That's a feature, not a hole: only put commands there that you trust.
- **Fix with AI** writes a prompt file in `.git/canopy/` and opens the
  assistant you chose. canopy doesn't call any AI service itself.
