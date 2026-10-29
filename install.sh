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
metadata_replaced=false
manifest_replaced=false
installation_complete=false
cleanup() {
  if [ "$installation_complete" = false ]; then
    if [ "$metadata_replaced" = true ]; then
      mv -f "$staged/previous-Info.plist" "$app_contents/Info.plist"
    fi
    if [ "$manifest_replaced" = true ]; then
      rm -f "$app_contents/Resources/build-info.json"
      if [ -f "$staged/previous-build-info.json" ]; then
        mv "$staged/previous-build-info.json" "$app_contents/Resources/build-info.json"
      fi
    fi
    if [ "$tools_replaced" = true ]; then
      rm -rf "$tools_dir"
      if [ -d "$staged/previous-tools" ]; then mv "$staged/previous-tools" "$tools_dir"; fi
    fi
  fi
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
install_name=${CUEMOL_INSTALL_NAME:-cuemol}
case "$install_name" in
  ''|.|..|*[!a-zA-Z0-9._-]*) printf "Invalid executable name\n" >&2; exit 1 ;;
esac
mkdir -p "$install_dir"
if [ -d "$install_dir/$install_name" ]; then
  printf "Installation target is a directory: %s/%s\n" "$install_dir" "$install_name" >&2
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
app_contents=${CUEMOL_APP_CONTENTS:-}
if [ -n "$app_contents" ] && [ "$(uname -s)" = Darwin ]; then
  app_version=$("$staged/cuemol" --version | sed -E 's/^cuemol v?([0-9]+\.[0-9]+\.[0-9]+).*/\1/')
  case "$app_version" in
    ''|*[!0-9.]*) printf 'Invalid application version: %s\n' "$app_version" >&2; exit 1 ;;
  esac
  cp "$app_contents/Info.plist" "$staged/previous-Info.plist"
  cp "$app_contents/Info.plist" "$staged/updated-Info.plist"
  /usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString $app_version" "$staged/updated-Info.plist"
  /usr/libexec/PlistBuddy -c "Set :CFBundleVersion $app_version" "$staged/updated-Info.plist"
  plutil -lint "$staged/updated-Info.plist" >/dev/null
fi
if [ -d "$staged/cuemol-tools" ]; then
  tools_dir=${CUEMOL_TOOLS_INSTALL_DIR:-"$install_dir/cuemol-tools"}
  mkdir -p "$(dirname "$tools_dir")"
  if [ -e "$tools_dir" ]; then mv "$tools_dir" "$staged/previous-tools"; fi
  tools_replaced=true
  mv "$staged/cuemol-tools" "$tools_dir"
fi
if [ -f "$staged/updated-Info.plist" ]; then
  metadata_replaced=true
  mv -f "$staged/updated-Info.plist" "$app_contents/Info.plist"
  if [ -f "$staged/build-info.json" ]; then
    if [ -f "$app_contents/Resources/build-info.json" ]; then
      cp "$app_contents/Resources/build-info.json" "$staged/previous-build-info.json"
    fi
    manifest_replaced=true
    mv -f "$staged/build-info.json" "$app_contents/Resources/build-info.json"
  fi
fi
mv -f "$staged/cuemol" "$install_dir/$install_name"
installation_complete=true
printf "Installed %s in %s\n" "$install_name" "$install_dir"
