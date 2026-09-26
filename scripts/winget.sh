#!/bin/sh
# Build the winget manifests for a release, and optionally submit them.
#
#   scripts/winget.sh             # render for the version in Cargo.toml
#   scripts/winget.sh 1.0.1       # render for a given version
#   scripts/winget.sh --submit    # render, then open a PR on microsoft/winget-pkgs
#
# Rendering reads the Windows zips' checksums from the GitHub release, so run
# it after the Release workflow has attached the builds. Output goes to
# target/winget/<version>/. Submitting uses your gh login: it forks
# microsoft/winget-pkgs (once), pushes the three files to a branch on your
# fork, and opens the PR. Nothing is cloned.
set -eu
cd "$(dirname "$0")/.."
die() { echo "winget: $*" >&2; exit 1; }

submit=no
version=""
for arg in "$@"; do
  case "$arg" in
    --submit) submit=yes ;;
    -h|--help) sed -n '2,12p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) version="${arg#v}" ;;
  esac
done
command -v gh >/dev/null 2>&1 || die "needs the GitHub CLI (gh), logged in"
[ -n "$version" ] || version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
[ -n "$version" ] || die "couldn't read the version from Cargo.toml"
tag="v$version"
id=ShankarS.Canopy

sha() {
  gh release download "$tag" --repo shankar-sachin/canopy -p "canopy-$tag-$1.zip.sha256" -O - 2>/dev/null |
    cut -d' ' -f1 | tr '[:lower:]' '[:upper:]'
}
sha_x64=$(sha x86_64-pc-windows-msvc)
sha_arm64=$(sha aarch64-pc-windows-msvc)
[ -n "$sha_x64" ] && [ -n "$sha_arm64" ] || die "$tag has no Windows builds yet (x64: ${sha_x64:-missing}, arm64: ${sha_arm64:-missing})"
date=$(gh release view "$tag" --repo shankar-sachin/canopy --json publishedAt -q '.publishedAt[0:10]')

out="target/winget/$version"
rm -rf "$out" && mkdir -p "$out"
for tpl in packaging/winget/*.yaml.in; do
  f=$(basename "$tpl" .in)
  sed -e "s|@VERSION@|$version|g" -e "s|@DATE@|$date|g" \
      -e "s|@SHA_X64@|$sha_x64|g" -e "s|@SHA_ARM64@|$sha_arm64|g" \
      -e '/^# Rendered by/d' "$tpl" > "$out/$f"
done
grep -l '@[A-Z_0-9]*@' "$out"/*.yaml && die "unfilled placeholders above"
echo "winget: manifests for $tag in $out"

[ "$submit" = yes ] || exit 0

me=$(gh api user -q .login)
fork="$me/winget-pkgs"
dir="manifests/s/ShankarS/Canopy/$version"
branch="canopy-$version"
if gh api "repos/microsoft/winget-pkgs/contents/manifests/s/ShankarS/Canopy" >/dev/null 2>&1; then
  title="New version: $id version $version"
else
  title="New package: $id version $version"
fi
gh api "repos/microsoft/winget-pkgs/contents/$dir" >/dev/null 2>&1 && die "$dir is already in winget-pkgs"

printf '%s to microsoft/winget-pkgs from %s? [y/N] ' "$title" "$fork"
read -r answer
[ "$answer" = y ] || [ "$answer" = Y ] || die "cancelled"

gh repo view "$fork" >/dev/null 2>&1 || gh repo fork microsoft/winget-pkgs --clone=false >/dev/null
gh repo sync "$fork" --branch master >/dev/null
base=$(gh api "repos/$fork/git/ref/heads/master" -q .object.sha)
gh api "repos/$fork/git/refs" -f "ref=refs/heads/$branch" -f "sha=$base" >/dev/null ||
  die "couldn't create $branch on $fork (delete it if it's left over from an earlier try)"
for f in "$out"/*.yaml; do
  gh api -X PUT "repos/$fork/contents/$dir/$(basename "$f")" \
    -f "message=$title" -f "branch=$branch" -f "content=$(base64 < "$f" | tr -d '\n')" >/dev/null
done
gh pr create --repo microsoft/winget-pkgs --head "$me:$branch" --title "$title" \
  --body "Canopy $tag: a git dashboard for the terminal (portable zip, x64 and arm64). Manifests generated from https://github.com/shankar-sachin/canopy/tree/main/packaging/winget."
