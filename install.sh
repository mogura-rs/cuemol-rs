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
tools_replaced=false
cleanup() {
  if [ -n "$staged" ]; then rm -rf "$staged"; fi
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
if [ -d "$install_dir/cuemol" ]; then
  printf "Installation target is a directory: %s/cuemol\n" "$install_dir" >&2
  exit 1
fi
staged=$(mktemp -d "$install_dir/.cuemol.XXXXXXXX")
if tar -tzf "$work/$asset" | awk '
  /(^\/|(^|\/)\.\.(\/|$))/ { bad = 1 }
  $0 != "cuemol" && $0 != "build-info.json" && $0 !~ /^cuemol-tools(\/|$)/ { bad = 1 }
  END { exit !bad }
'; then
  printf "Invalid release archive paths\n" >&2
  exit 1
fi
tar -xzf "$work/$asset" -C "$staged"
test -f "$staged/cuemol"
chmod 755 "$staged/cuemol"
if [ -d "$staged/cuemol-tools" ]; then
  tools_dir=${CUEMOL_TOOLS_INSTALL_DIR:-"$install_dir/cuemol-tools"}
  if [ -e "$tools_dir" ]; then mv "$tools_dir" "$staged/previous-tools"; fi
  if ! mv "$staged/cuemol-tools" "$tools_dir"; then
    if [ -d "$staged/previous-tools" ]; then mv "$staged/previous-tools" "$tools_dir"; fi
    exit 1
  fi
  tools_replaced=true
fi
if ! mv -f "$staged/cuemol" "$install_dir/cuemol"; then
  if [ "$tools_replaced" = true ]; then
    rm -rf "$tools_dir"
    if [ -d "$staged/previous-tools" ]; then mv "$staged/previous-tools" "$tools_dir"; fi
  fi
  exit 1
fi
printf "Installed cuemol in %s\n" "$install_dir"
