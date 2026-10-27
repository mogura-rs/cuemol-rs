#!/bin/sh
set -eu

repository=${CUEMOL_RELEASE_REPOSITORY:-mogura-rs/cuemol2-rs}
version=${CUEMOL_VERSION:-latest}
case "$version" in
  latest) release_url="https://github.com/$repository/releases/latest/download" ;;
  *[!a-zA-Z0-9._-]*|"") printf "Invalid release version\n" >&2; exit 1 ;;
  *) release_url="https://github.com/$repository/releases/download/$version" ;;
esac
release_url=${CUEMOL_RELEASE_URL:-$release_url}
case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) target=x86_64-unknown-linux-gnu ;;
  Linux-aarch64|Linux-arm64) target=aarch64-unknown-linux-gnu ;;
  Darwin-arm64) target=aarch64-apple-darwin ;;
  Darwin-x86_64) target=x86_64-apple-darwin ;;
  *) printf "Unsupported platform: %s %s\n" "$(uname -s)" "$(uname -m)" >&2; exit 1 ;;
esac
asset="cuemol-$target.tar.gz"
work=$(mktemp -d "${TMPDIR:-/tmp}/cuemol-install.XXXXXXXX")
staged=
cleanup() {
  if [ -n "$staged" ]; then rm -f "$staged"; fi
  rm -rf "$work"
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM
curl -fsSL "$release_url/$asset" -o "$work/$asset"
curl -fsSL "$release_url/SHA256SUMS" -o "$work/SHA256SUMS"
expected=$(awk -v name="$asset" '$2 == name { print $1 }' "$work/SHA256SUMS")
case "$expected" in
  *[!0123456789abcdefABCDEF]*|"") printf "Missing or invalid checksum\n" >&2; exit 1 ;;
esac
if [ "${#expected}" -ne 64 ]; then printf "Invalid checksum length\n" >&2; exit 1; fi
if command -v sha256sum >/dev/null 2>&1; then
  actual=$(sha256sum "$work/$asset" | awk '{print $1}')
elif command -v shasum >/dev/null 2>&1; then
  actual=$(shasum -a 256 "$work/$asset" | awk '{print $1}')
else
  printf "sha256sum or shasum is required\n" >&2; exit 1
fi
if [ "$actual" != "$expected" ]; then printf "Checksum mismatch for %s\n" "$asset" >&2; exit 1; fi
install_dir=${CUEMOL_INSTALL_DIR:-"${HOME:?HOME is required}/.local/bin"}
mkdir -p "$install_dir"
staged=$(mktemp "$install_dir/.cuemol.XXXXXXXX")
tar -xOzf "$work/$asset" cuemol > "$staged"
chmod 755 "$staged"
mv -f "$staged" "$install_dir/cuemol"
staged=
printf "Installed cuemol in %s\n" "$install_dir"
