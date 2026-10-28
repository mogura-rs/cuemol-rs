#!/usr/bin/env bash
set -euo pipefail
: "${RELEASE_TAG:?Set RELEASE_TAG}"
: "${GH_REPO:?Set GH_REPO}"
[[ "$RELEASE_TAG" =~ ^v[0-9]+\.[0-9]+\.[0-9]+([-.][0-9A-Za-z.-]+)?$ ]]
directory=${1:-dist}
cd "$directory"
sha256sum --check SHA256SUMS
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
if gh release view "$RELEASE_TAG" --json isDraft --jq .isDraft > "$work/draft"; then
  if [ "$(cat "$work/draft")" != true ]; then
    echo 'This version is already published; create a new version tag.' >&2
    exit 1
  fi
else
  args=()
  if [[ "$RELEASE_TAG" == *-* ]]; then args+=(--prerelease); fi
  gh release create "$RELEASE_TAG" --verify-tag --generate-notes --draft "${args[@]}"
fi
gh release upload "$RELEASE_TAG" --clobber ./*
gh release view "$RELEASE_TAG" --json assets --jq '.assets[].name' | LC_ALL=C sort > "$work/actual"
{ awk '{print $2}' SHA256SUMS; echo SHA256SUMS; } | LC_ALL=C sort > "$work/expected"
diff -u "$work/expected" "$work/actual"
gh release download "$RELEASE_TAG" --pattern SHA256SUMS --dir "$work"
cmp SHA256SUMS "$work/SHA256SUMS"
gh release edit "$RELEASE_TAG" --draft=false
