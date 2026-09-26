#!/bin/sh
# Tag and publish a release, credited to you (not github-actions).
#
#   scripts/release.sh            # release the version in Cargo.toml
#
# 1. Checks you're on an up-to-date, clean main and that vX.Y.Z is new.
# 2. Runs scripts/check.sh.
# 3. Pushes an annotated tag and creates the GitHub release with your gh login.
# 4. Waits for the Release workflow (builds + Homebrew tap) and verifies the
#    tap's checksum.
#
# Bump `version` in Cargo.toml (and add a changelog entry) in a PR first.
set -eu
cd "$(dirname "$0")/.."
die() { echo "release: $*" >&2; exit 1; }

command -v gh >/dev/null 2>&1 || die "needs the GitHub CLI (gh), logged in"
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
[ -n "$version" ] || die "couldn't read the version from Cargo.toml"
tag="v$version"

[ "$(git branch --show-current)" = main ] || die "switch to main first"
[ -z "$(git status --porcelain --untracked-files=no)" ] || die "commit or stash your changes first"
git fetch -q origin main --tags
[ "$(git rev-parse HEAD)" = "$(git rev-parse origin/main)" ] || die "main isn't in sync with origin (git pull)"
git rev-parse -q --verify "refs/tags/$tag" >/dev/null && die "$tag already exists; bump the version in Cargo.toml"

scripts/check.sh

printf 'Release %s? [y/N] ' "$tag"
read -r answer
[ "$answer" = y ] || [ "$answer" = Y ] || die "cancelled"

git tag -a "$tag" -m "Canopy $tag"
git push -q origin "$tag"
gh release create "$tag" --verify-tag --title "Canopy $tag" --generate-notes
echo "release: created $tag as $(gh api user -q .login)"

echo "release: waiting for the Release workflow..."
sleep 10
run=$(gh run list --workflow release.yml --branch "$tag" -L 1 --json databaseId -q '.[0].databaseId')
gh run watch "$run" --exit-status >/dev/null || { gh run view "$run"; die "the Release workflow failed"; }

repo=$(gh repo view --json nameWithOwner -q .nameWithOwner)
want=$(curl -sL "https://github.com/$repo/archive/refs/tags/$tag.tar.gz" | shasum -a 256 | cut -d' ' -f1)
got=$(gh api repos/shankar-sachin/homebrew-canopy/contents/Formula/canopy.rb -q .content | base64 -d | sed -n 's/.*sha256 "\(.*\)"/\1/p')
[ "$want" = "$got" ] || die "the tap's checksum ($got) doesn't match the source tarball ($want)"
echo "release: $tag is out: https://github.com/$repo/releases/tag/$tag"
gh release view "$tag" --json assets -q '.assets[].name' | sed 's/^/  /'
