#!/bin/sh
# Install canopy desktop from a GitHub release, without Homebrew or a browser.
#
#   curl -fsSL https://gitcanopy.netlify.app/install-desktop.sh | sh
#   curl -fsSL https://gitcanopy.netlify.app/install-desktop.sh | sh -s -- v1.0.0   # a specific version
#   curl -fsSL https://gitcanopy.netlify.app/install-desktop.sh | sh -s -- --uninstall
#
# macOS (Apple silicon and Intel): puts canopy.app in /Applications, or ~/Applications
# if that isn't writable (override with CANOPY_APP_DIR).
# Linux (x86_64): puts an AppImage in ~/.local/bin (override with CANOPY_INSTALL_DIR). No sudo.
# On Windows use install-desktop.ps1.
set -eu

REPO="shankar-sachin/canopy"
VERSION="latest"

say() { printf '%s\n' "canopy: $*"; }
die() { printf '%s\n' "canopy: $*" >&2; exit 1; }

OS=$(uname -s)
APPDIR="${CANOPY_APP_DIR:-}"
if [ "$OS" = Darwin ] && [ -z "$APPDIR" ]; then
  if [ -w /Applications ]; then APPDIR=/Applications; else APPDIR="$HOME/Applications"; fi
fi
BINDIR="${CANOPY_INSTALL_DIR:-$HOME/.local/bin}"

for arg in "$@"; do
  case "$arg" in
    --uninstall)
      if [ "$OS" = Darwin ]; then
        for d in "$APPDIR" /Applications "$HOME/Applications"; do
          if [ -d "$d/canopy.app" ]; then rm -rf "$d/canopy.app"; say "removed $d/canopy.app"; fi
        done
      else
        rm -f "$BINDIR/canopy-desktop.AppImage" && say "removed $BINDIR/canopy-desktop.AppImage"
      fi
      say "your config (~/.canopy) was left alone"
      exit 0 ;;
    -h|--help) sed -n '2,11p' "$0" 2>/dev/null || true; exit 0 ;;
    v*) VERSION="$arg" ;;
    *) die "unknown argument: $arg (try a version like v1.0.0, or --uninstall)" ;;
  esac
done

case "$OS-$(uname -m)" in
  Darwin-arm64) suffix=aarch64-apple-darwin.dmg ;;
  Darwin-x86_64) suffix=x86_64-apple-darwin.dmg ;;
  Linux-x86_64 | Linux-amd64) suffix=x86_64.AppImage ;;
  *) die "no prebuilt desktop app for $OS $(uname -m); see https://github.com/$REPO/releases" ;;
esac

command -v curl >/dev/null 2>&1 || die "curl is required"

if [ "$VERSION" = latest ]; then
  VERSION=$(curl -fsSLI -o /dev/null -w '%{url_effective}' "https://github.com/$REPO/releases/latest")
  VERSION=${VERSION##*/}
  case "$VERSION" in v*) ;; *) die "couldn't find the latest release" ;; esac
fi

name="canopy-desktop-$VERSION-$suffix"
url="https://github.com/$REPO/releases/download/$VERSION/$name"
tmp=$(mktemp -d)
mnt=""
cleanup() { [ -n "$mnt" ] && hdiutil detach -quiet "$mnt" >/dev/null 2>&1; rm -rf "$tmp"; }
trap cleanup EXIT INT TERM

say "downloading $name"
curl -fsSL "$url" -o "$tmp/$name" || die "download failed: $url"
curl -fsSL "$url.sha256" -o "$tmp/$name.sha256" || die "checksum download failed"

want=$(cut -d' ' -f1 <"$tmp/$name.sha256")
if command -v shasum >/dev/null 2>&1; then
  got=$(shasum -a 256 "$tmp/$name" | cut -d' ' -f1)
else
  got=$(sha256sum "$tmp/$name" | cut -d' ' -f1)
fi
[ "$want" = "$got" ] || die "checksum mismatch (expected $want, got $got); not installing"

if [ "$OS" = Darwin ]; then
  mnt="$tmp/mnt"
  mkdir -p "$mnt"
  hdiutil attach -quiet -nobrowse -readonly -mountpoint "$mnt" "$tmp/$name" >/dev/null || die "couldn't open the disk image"
  [ -d "$mnt/canopy.app" ] || die "canopy.app not found in the disk image"
  mkdir -p "$APPDIR"
  rm -rf "$APPDIR/canopy.app"
  cp -R "$mnt/canopy.app" "$APPDIR/canopy.app"
  # Downloaded with curl, so macOS hasn't quarantined it; clear the flag anyway.
  xattr -dr com.apple.quarantine "$APPDIR/canopy.app" 2>/dev/null || true
  say "installed $VERSION to $APPDIR/canopy.app"
  say "open it from Launchpad or Spotlight, or: open \"$APPDIR/canopy.app\""
else
  mkdir -p "$BINDIR"
  install -m 755 "$tmp/$name" "$BINDIR/canopy-desktop.AppImage" 2>/dev/null || { cp "$tmp/$name" "$BINDIR/canopy-desktop.AppImage" && chmod 755 "$BINDIR/canopy-desktop.AppImage"; }
  say "installed $VERSION to $BINDIR/canopy-desktop.AppImage"
  say "run: $BINDIR/canopy-desktop.AppImage   (needs FUSE 2; if it won't start, add --appimage-extract-and-run)"
fi
command -v git >/dev/null 2>&1 || say "note: canopy needs git"
