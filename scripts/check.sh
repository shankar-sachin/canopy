#!/bin/sh
# Everything CI checks: formatting, clippy (warnings are errors), and tests.
# Prints a one-line verdict and exits non-zero on the first failure.
#
#   scripts/check.sh
set -eu
cd "$(dirname "$0")/.."
log=$(mktemp -d)
trap 'rm -rf "$log"' EXIT

cargo fmt --all --check || { echo "VERDICT: fmt FAILED (run: cargo fmt --all)"; exit 1; }
cargo clippy --workspace --all-targets -- -D warnings >"$log/clippy" 2>&1 || {
  grep -E "^(error|warning)" -A8 "$log/clippy" | head -40
  echo "VERDICT: clippy FAILED"; exit 1
}
cargo test --workspace >"$log/test" 2>&1 || {
  grep -E "panicked|FAILED" -A10 "$log/test" | head -40
  echo "VERDICT: tests FAILED"; exit 1
}
grep -E "^test result" "$log/test" | awk '{p+=$4} END {print p " tests passed"}'
echo "VERDICT: ALL GREEN"
