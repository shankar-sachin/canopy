#!/bin/sh
# The full audit, before a release or after a big change: everything CI
# checks, plus the JavaScript, the website and the release bookkeeping.
#
#   scripts/audit.sh           # stop at the first failing step
#   scripts/audit.sh --quick   # skip the Rust build/test step (check.sh)
#
# Steps:
#   1. scripts/check.sh          fmt, clippy, every Rust test
#   2. scripts/test-ui.sh        syntax of every script, desktop UI tests
#   3. scripts/audit-site.mjs    the website: links, anchors, head tags, index
#   4. versions                  Cargo.toml, Cargo.lock and the site agree
#   5. shell scripts             sh -n on scripts/*.sh
#   6. hygiene                   no debug prints or stray TODOs in shipped code
set -eu
cd "$(dirname "$0")/.."
quick=false
[ "${1:-}" = --quick ] && quick=true
step() { printf '\n== %s\n' "$*"; }
fail() { echo "AUDIT FAILED: $*"; exit 1; }

if [ "$quick" = false ]; then
  step "1/7 Rust: fmt, clippy, tests"
  scripts/check.sh || fail "scripts/check.sh"
fi

step "2/7 JavaScript"
scripts/test-ui.sh || fail "scripts/test-ui.sh"

step "3/7 website"
node scripts/audit-site.mjs || fail "the website (node scripts/audit-site.mjs)"

step "4/7 third-party licenses"
node scripts/third-party.mjs --check || fail "third-party licenses (node scripts/third-party.mjs)"

step "5/7 versions"
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
echo "Cargo.toml: $version"
# Every workspace crate in Cargo.lock carries the same version.
for c in canopy-git-tui canopy-git canopy-gh canopy-config canopy-highlight canopy-desktop; do
  got=$(awk -v n="$c" '$0 == "name = \"" n "\"" { getline; gsub(/version = |"/, ""); print; exit }' Cargo.lock)
  [ "$got" = "$version" ] || fail "Cargo.lock has $c $got, not $version (run cargo build)"
done
echo "Cargo.lock: every canopy crate is $version"
grep -q "<b>v$version</b>" docs/index.html || fail "the roadmap on docs/index.html doesn't show v$version"
echo "docs/index.html roadmap: v$version"

step "6/7 shell scripts"
for f in scripts/*.sh; do sh -n "$f" || fail "syntax error in $f"; done
echo "$(ls scripts/*.sh | wc -l | tr -d ' ') scripts parse"

step "7/7 hygiene"
# Debug output left in shipped code (tests and the CLI's own println! are fine).
if grep -rnE 'dbg!\(' crates --include='*.rs' | grep -v '/target/'; then fail "dbg!() left in"; fi
if grep -nE 'console\.log\(' crates/canopy-gui/ui/*.js docs/assets/*.js; then fail "console.log left in shipped JavaScript"; fi
todos=$(grep -rnE '(//|#|/\*) *(TODO|FIXME|XXX)\b' crates docs/assets scripts --include='*.rs' --include='*.js' --include='*.sh' --include='*.mjs' | grep -v '/target/' | grep -v 'scripts/audit.sh' || true)
if [ -n "$todos" ]; then echo "$todos"; echo "(TODOs are listed, not fatal)"; else echo "no TODO/FIXME"; fi

printf '\nAUDIT: ALL GREEN\n'
