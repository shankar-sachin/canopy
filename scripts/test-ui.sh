#!/bin/sh
# Test the JavaScript: the desktop app's front end (crates/canopy-gui/ui) and
# the website's scripts (docs/assets). Needs Node 20 or later, and nothing
# from npm.
#
#   scripts/test-ui.sh
#
# 1. Syntax-checks every script (node --check).
# 2. Runs the desktop UI tests in crates/canopy-gui/ui-tests, which load the
#    real ui/*.js into a sandbox and check the pages they draw.
set -eu
cd "$(dirname "$0")/.."
command -v node >/dev/null 2>&1 || { echo "test-ui: needs Node.js 20 or later (https://nodejs.org)"; exit 1; }
major=$(node -p 'process.versions.node.split(".")[0]')
[ "$major" -ge 20 ] || { echo "test-ui: needs Node.js 20 or later (this is $(node --version))"; exit 1; }

n=0
for f in crates/canopy-gui/ui/*.js docs/assets/*.js scripts/*.mjs crates/canopy-gui/ui-tests/*.mjs; do
  node --check "$f" || { echo "VERDICT: syntax error in $f"; exit 1; }
  n=$((n + 1))
done
echo "test-ui: $n scripts parse"

node --test crates/canopy-gui/ui-tests/ >"${TMPDIR:-/tmp}/canopy-ui-tests.log" 2>&1 || {
  grep -E "^not ok|✖|Error|expected|actual" "${TMPDIR:-/tmp}/canopy-ui-tests.log" | head -40
  echo "VERDICT: UI tests FAILED (full log: ${TMPDIR:-/tmp}/canopy-ui-tests.log)"
  exit 1
}
grep -E "^ℹ (tests|pass)" "${TMPDIR:-/tmp}/canopy-ui-tests.log" | tr '\n' ' '
echo
echo "VERDICT: UI tests passed"
