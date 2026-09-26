#!/bin/sh
# Regenerate the website's terminal screenshots and the generated key and
# theme tables in docs/ from the real TUI (rendered off-screen, no terminal
# needed). Run after changing anything visible, then commit docs/.
#
#   scripts/screens.sh
set -eu
cd "$(dirname "$0")/.."
cargo test -q -p canopy-git-tui export_site_screens -- --ignored
git status --short docs
