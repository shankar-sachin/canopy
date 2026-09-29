#!/bin/sh
# Retake the README's terminal picture (docs/assets/terminal/home.png) from the
# frame the website already shows (filled in by scripts/screens.sh), drawn with
# the site's own stylesheet in headless Chrome.
#
#   scripts/screens.sh && scripts/terminal-shots.sh
#
# Needs Google Chrome or Chromium (set CHROME=/path/to/chrome if it isn't found).
set -eu
cd "$(dirname "$0")/.."
root=$(pwd)

if [ -z "${CHROME:-}" ]; then
  for c in "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" "/Applications/Chromium.app/Contents/MacOS/Chromium" \
    google-chrome google-chrome-stable chromium chromium-browser; do
    if [ -x "$c" ] || command -v "$c" >/dev/null 2>&1; then CHROME=$c; break; fi
  done
fi
[ -n "${CHROME:-}" ] || { echo "terminal-shots: Chrome not found; set CHROME=/path/to/chrome"; exit 1; }

tmp=$(cd "$(mktemp -d)" && pwd -P)
trap 'rm -rf "$tmp"' EXIT
out=$root/docs/assets/terminal
mkdir -p "$out"

node -e '
const fs = require("fs");
const root = process.argv[1];
const page = fs.readFileSync(root + "/docs/index.html", "utf8");
const m = page.match(/<!-- SCREEN:home -->([\s\S]*?)<!-- \/SCREEN:home -->/);
if (!m || !m[1].includes("<span")) { console.error("terminal-shots: run scripts/screens.sh first (docs/index.html has no home frame)"); process.exit(1); }
fs.writeFileSync(process.argv[2], `<!doctype html><meta charset="utf-8">
<link rel="stylesheet" href="file://${root}/docs/assets/site.css">
<link href="https://fonts.googleapis.com/css2?family=JetBrains+Mono:wght@400;700&display=swap" rel="stylesheet">
<style>html,body{margin:0;background:#0b100c}body{padding:28px;width:max-content}</style>
<div class="window"><div class="chrome"><i></i><i></i><i></i><span>canopy: Home</span></div><div class="screen">${m[1]}</div></div>`);
' "$root" "$tmp/home.html"

rm -f "$out/home.png"
"$CHROME" --headless=new --disable-gpu --hide-scrollbars --no-first-run --no-default-browser-check \
  --user-data-dir="$tmp/profile" --force-device-scale-factor=2 --window-size=${WIDTH:-940},${HEIGHT:-596} \
  --virtual-time-budget=4000 --screenshot="$out/home.png" "file://$tmp/home.html" >/dev/null 2>&1 &
pid=$!
i=0
while [ ! -s "$out/home.png" ] && [ $i -lt 120 ] && kill -0 $pid 2>/dev/null; do sleep 0.5; i=$((i + 1)); done
sleep 1
kill $pid 2>/dev/null || true
wait $pid 2>/dev/null || true
[ -s "$out/home.png" ] || { echo "terminal-shots: no picture"; exit 1; }
echo "  home.png"
