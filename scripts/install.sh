#!/bin/sh
# Install Canopy from a GitHub release, without Homebrew.
#
#   curl -fsSL https://raw.githubusercontent.com/shankar-sachin/canopy/main/scripts/install.sh | sh
#   curl -fsSL .../install.sh | sh -s -- v1.0.0       # a specific version
#   curl -fsSL .../install.sh | sh -s -- --uninstall
#
# Installs to ~/.local/bin (override with CANOPY_INSTALL_DIR). No sudo.
# macOS (Apple silicon and Intel) and Linux x86_64. On Windows use install.ps1.
set -eu

REPO="shankar-sachin/canopy"
DIR="${CANOPY_INSTALL_DIR:-$HOME/.local/bin}"
VERSION="latest"

say() { printf '%s\n' "canopy: $*"; }
die() { printf '%s\n' "canopy: $*" >&2; exit 1; }

for arg in "$@"; do
  case "$arg" in
    --uninstall)
      if [ -e "$DIR/canopy" ]; then rm -f "$DIR/canopy"; say "removed $DIR/canopy"; else say "nothing to remove in $DIR"; fi
      say "your config (~/.config/canopy) was left alone"
      exit 0 ;;
    -h|--help) sed -n '2,10p' "$0" 2>/dev/null || true; exit 0 ;;
    v*) VERSION="$arg" ;;
    *) die "unknown argument: $arg (try a version like v1.0.0, or --uninstall)" ;;
  esac
done

case "$(uname -s)-$(uname -m)" in
  Darwin-arm64) target=aarch64-apple-darwin ;;
  Darwin-x86_64) target=x86_64-apple-darwin ;;
  Linux-x86_64 | Linux-amd64) target=x86_64-unknown-linux-gnu ;;
  *) die "no prebuilt binary for $(uname -s) $(uname -m); build from source: cargo install --locked --git https://github.com/$REPO canopy-git-tui" ;;
esac

command -v curl >/dev/null 2>&1 || die "curl is required"
command -v tar >/dev/null 2>&1 || die "tar is required"

if [ "$VERSION" = latest ]; then
  # The /releases/latest page redirects to /releases/tag/<version>.
  VERSION=$(curl -fsSLI -o /dev/null -w '%{url_effective}' "https://github.com/$REPO/releases/latest")
  VERSION=${VERSION##*/}
  case "$VERSION" in v*) ;; *) die "couldn't find the latest release" ;; esac
fi

name="canopy-$VERSION-$target"
url="https://github.com/$REPO/releases/download/$VERSION/$name.tar.gz"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT INT TERM

say "downloading $name"
curl -fsSL "$url" -o "$tmp/$name.tar.gz" || die "download failed: $url"
curl -fsSL "$url.sha256" -o "$tmp/$name.tar.gz.sha256" || die "checksum download failed"

want=$(cut -d' ' -f1 <"$tmp/$name.tar.gz.sha256")
if command -v shasum >/dev/null 2>&1; then
  got=$(shasum -a 256 "$tmp/$name.tar.gz" | cut -d' ' -f1)
else
  got=$(sha256sum "$tmp/$name.tar.gz" | cut -d' ' -f1)
fi
[ "$want" = "$got" ] || die "checksum mismatch (expected $want, got $got); not installing"

tar -xzf "$tmp/$name.tar.gz" -C "$tmp"
mkdir -p "$DIR"
install -m 755 "$tmp/$name/canopy" "$DIR/canopy" 2>/dev/null || { cp "$tmp/$name/canopy" "$DIR/canopy" && chmod 755 "$DIR/canopy"; }
say "installed $VERSION to $DIR/canopy"

case ":$PATH:" in
  *":$DIR:"*) say "run: canopy" ;;
  *) say "$DIR isn't on your PATH yet; add this to your shell profile:"
     printf '\n    export PATH="%s:$PATH"\n\n' "$DIR" ;;
esac
command -v git >/dev/null 2>&1 || say "note: Canopy needs git"
