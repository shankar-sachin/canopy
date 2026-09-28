#!/bin/sh
# Where the tests are, and where they aren't: a quick map to find gaps
# (no coverage tool needed).
#
#   scripts/test-map.sh
#
# Prints tests per crate, the Rust files with no #[test] of their own (some
# are covered from elsewhere: the TUI's frame tests in ui_tests.rs, git ops
# in canopy-git/tests), and the canopy-git operations no test calls.
set -eu
cd "$(dirname "$0")/.."

echo "== tests per crate"
for c in crates/*/; do
  n=$(grep -rhoE '#\[(tokio::)?test\]' "$c" --include='*.rs' 2>/dev/null | wc -l | tr -d ' ')
  i=$(grep -rhE '#\[ignore' "$c" --include='*.rs' 2>/dev/null | wc -l | tr -d ' ')
  printf '  %-22s %4s tests (%s ignored)\n' "$(basename "$c")" "$n" "$i"
done
js=$(grep -hoE '^test\(' crates/canopy-gui/ui-tests/*.mjs 2>/dev/null | wc -l | tr -d ' ')
printf '  %-22s %4s tests\n' "desktop UI (JS)" "$js"

echo
echo "== Rust files with no tests of their own (lines)"
find crates -name '*.rs' -not -path '*/target/*' -not -path '*/tests/*' | sort | while read -r f; do
  if ! grep -qE '#\[(tokio::)?test\]' "$f"; then
    l=$(wc -l <"$f" | tr -d ' ')
    [ "$l" -ge 60 ] && printf '  %5s  %s\n' "$l" "$f"
  fi
done

echo
echo "== canopy-git operations no test calls"
# Called from a test file, or from a #[cfg(test)] module inside src.
for f in $(grep -hoE 'pub async fn [a-z_]+' crates/canopy-git/src/ops.rs crates/canopy-git/src/init.rs | awk '{print $4}' | sort -u); do
  hit=$(grep -rlE "\.$f\(|::$f\(" crates/canopy-git/tests crates/canopy-git/src 2>/dev/null | while read -r file; do
    if echo "$file" | grep -q /tests/; then echo "$file"
    elif awk '/#\[cfg\(test\)\]/{t=1} t' "$file" | grep -qE "\.$f\(|::$f\("; then echo "$file"; fi
  done)
  [ -n "$hit" ] || printf '  %s\n' "$f"
done
