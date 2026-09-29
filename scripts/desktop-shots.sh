#!/bin/sh
# Retake the website's and README's desktop-app screenshots
# (docs/assets/desktop/*.png) without launching the app: the real front end
# (crates/canopy-gui/ui) runs in headless Chrome, and a stand-in for the Rust
# side (scripts/desktop-shots/mock.js) answers it with real git data from a
# demo repository (dumped by the dump_overview test) plus made-up GitHub data.
#
#   scripts/desktop-shots.sh                 # every scene
#   scripts/desktop-shots.sh changes stash   # just these
#
# Needs Google Chrome or Chromium (set CHROME=/path/to/chrome if it isn't
# found). Pictures are 1280x800 at 2x. Commit docs/assets after checking them.
set -eu
cd "$(dirname "$0")/.."
root=$(pwd)

SCENES="home home-light learning welcome changes amend history reset branches tags stash conflict undo diverged setup connect prs review runs fix-ci notifications settings settings-general"
[ $# -gt 0 ] && SCENES="$*"

if [ -z "${CHROME:-}" ]; then
  for c in "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" "/Applications/Chromium.app/Contents/MacOS/Chromium" \
    google-chrome google-chrome-stable chromium chromium-browser; do
    if [ -x "$c" ] || command -v "$c" >/dev/null 2>&1; then CHROME=$c; break; fi
  done
fi
[ -n "${CHROME:-}" ] || { echo "desktop-shots: Chrome not found; set CHROME=/path/to/chrome"; exit 1; }

tmp=$(cd "$(mktemp -d)" && pwd -P)
# KEEP=1 keeps the work folder, to open the scenes in a browser yourself.
if [ -n "${KEEP:-}" ]; then echo "desktop-shots: working in $tmp"; else trap 'rm -rf "$tmp"' EXIT; fi
quiet() { "$@" >/dev/null 2>&1; }
at() { d=$1; shift; (cd "$d" && "$@"); }
past() { echo "$(($(date +%s) - $1 * 3600)) +0000"; }

# The main repository: acme-app on GitHub (a local bare repo stands in), one
# commit not pushed yet, a stash, and uncommitted edits to src/main.rs.
main=$tmp/acme-app
scripts/demo-repo.sh "$main" >/dev/null
at "$main" git init -q --bare "$tmp/origin.git"
at "$main" git remote add origin "$tmp/origin.git"
at "$main" git stash -q
at "$main" quiet git push -u origin main feature/top-words
at "$main" git branch -q --set-upstream-to=origin/feature/top-words feature/top-words
printf '# Changelog\n\n## 0.2.0\n\n- Count words case-insensitively.\n' >"$main/CHANGELOG.md"
at "$main" git add CHANGELOG.md
GIT_AUTHOR_DATE=$(past 2) GIT_COMMITTER_DATE=$(past 2) at "$main" git commit -q -m "Start a changelog"
printf '# acme-app\n\nCounts the words in a file.\n\n## Usage\n\n    acme-app notes.txt\n' >"$main/README.md"
at "$main" git stash push -q -m "Usage section for the README" README.md
at "$main" git stash pop -q stash@{1}

# A second copy, stopped in the middle of a merge with a conflict.
merge=$tmp/merge/acme-app
scripts/demo-repo.sh "$merge" >/dev/null
(
  cd "$merge"
  git stash -q
  git switch -q -c polite
  sed 's/different words/different words, thank you!/' src/main.rs >src/main.rs.new && mv src/main.rs.new src/main.rs
  git commit -q -am "Say thank you"
  git switch -q main
  sed 's/different words/distinct words/' src/main.rs >src/main.rs.new && mv src/main.rs.new src/main.rs
  git commit -q -am "Say distinct, not different"
  quiet git merge polite || true
)

echo "desktop-shots: reading the repositories"
dump() { CANOPY_OVERVIEW_OUT=$2 CANOPY_REPO=$1 cargo test -q -p canopy-desktop dump_overview -- --ignored >/dev/null 2>&1; }
dump "$main" "$tmp/main.json"
dump "$merge" "$tmp/merge.json"
# Friendly paths in the pictures, not the temporary folder.
for f in main merge; do
  sed -e "s|$tmp/merge/|/Users/ada/code/|g" -e "s|$tmp/|/Users/ada/code/|g" "$tmp/$f.json" >"$tmp/$f.fixed" && mv "$tmp/$f.fixed" "$tmp/$f.json"
done

# The front end, with the stand-in loaded before it.
cp -R crates/canopy-gui/ui "$tmp/ui"
cp scripts/desktop-shots/mock.js "$tmp/ui/mock.js"
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
next=$(echo "$version" | awk -F. '{print $1"."$2"."$3+1}')
make_data() {
  { printf 'window.MOCK_DATA = { version: "%s", next_version: "%s", theme: "%s", main: ' "$version" "$next" "$1"
    cat "$tmp/main.json"; printf ', merge: '; cat "$tmp/merge.json"; printf ' };\n'; } >"$tmp/ui/data.js"
}
sed 's|<script src="brands.js"></script>|<script src="data.js"></script><script src="mock.js"></script><script src="brands.js"></script>|' \
  crates/canopy-gui/ui/index.html >"$tmp/ui/index.html"

out=$root/docs/assets/desktop
mkdir -p "$out"
for scene in $SCENES; do
  case $scene in
    *-light) make_data light; name=${scene%-light} ;;
    *) make_data dark; name=$scene ;;
  esac
  # Chrome writes the picture within a few seconds but doesn't always exit
  # afterwards, so wait for the file, then close it.
  rm -f "$out/$scene.png"
  "$CHROME" --headless=new --disable-gpu --hide-scrollbars --no-first-run --no-default-browser-check \
    --user-data-dir="$tmp/profile" --force-device-scale-factor=2 --window-size=1280,800 \
    --virtual-time-budget=4000 --screenshot="$out/$scene.png" "file://$tmp/ui/index.html#$name" >/dev/null 2>&1 &
  pid=$!
  i=0
  while [ ! -s "$out/$scene.png" ] && [ $i -lt 120 ] && kill -0 $pid 2>/dev/null; do sleep 0.5; i=$((i + 1)); done
  sleep 1
  kill $pid 2>/dev/null || true
  wait $pid 2>/dev/null || true
  [ -s "$out/$scene.png" ] || { echo "desktop-shots: no picture for $scene"; exit 1; }
  echo "  $scene.png"
done
git status --short docs/assets/desktop
