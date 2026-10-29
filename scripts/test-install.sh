#!/bin/sh
set -eu
root=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
work=$(mktemp -d "${TMPDIR:-/tmp}/cuemol-install-test.XXXXXXXX")
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/release" "$work/input" "$work/bin"
case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) target=x86_64-unknown-linux-gnu ;;
  Linux-aarch64|Linux-arm64) target=aarch64-unknown-linux-gnu ;;
  Darwin-arm64) target=aarch64-apple-darwin ;;
  Darwin-x86_64) target=x86_64-apple-darwin ;;
  *) exit 1 ;;
esac
cat > "$work/input/cuemol" <<'SH'
#!/bin/sh
if [ "${1:-}" = --version ]; then printf 'cuemol v1.2.3\n'; else printf 'installed\n'; fi
SH
printf '{"version":"test"}\n' > "$work/input/build-info.json"
mkdir -p "$work/input/cuemol-tools/apbs/bin"
printf 'bundled\n' > "$work/input/cuemol-tools/apbs/bin/apbs"
tar -czf "$work/release/cuemol-$target.tar.gz" -C "$work/input" cuemol cuemol-tools build-info.json
(cd "$work/release" && shasum -a 256 "cuemol-$target.tar.gz" > SHA256SUMS)
CUEMOL_RELEASE_URL="file://$work/release" CUEMOL_INSTALL_DIR="$work/bin" sh "$root/install.sh"
test "$("$work/bin/cuemol")" = installed
test "$(cat "$work/bin/cuemol-tools/apbs/bin/apbs")" = bundled
printf '%064d  cuemol-%s.tar.gz\n' 0 "$target" > "$work/release/SHA256SUMS"
if CUEMOL_RELEASE_URL="file://$work/release" CUEMOL_INSTALL_DIR="$work/bin" sh "$root/install.sh"; then
  printf 'Installer accepted a bad checksum\n' >&2
  exit 1
fi
test "$("$work/bin/cuemol")" = installed
test "$(cat "$work/bin/cuemol-tools/apbs/bin/apbs")" = bundled
(cd "$work/release" && shasum -a 256 "cuemol-$target.tar.gz" > SHA256SUMS)
real_mv=$(command -v mv)
mkdir -p "$work/commands"
cat > "$work/commands/mv" <<'SH'
#!/bin/sh
for argument do destination=$argument; done
if [ "$destination" = "$CUEMOL_TEST_DESTINATION" ]; then exit 1; fi
exec "$CUEMOL_TEST_MV" "$@"
SH
chmod 755 "$work/commands/mv"
printf 'previous\n' > "$work/bin/cuemol-tools/apbs/bin/apbs"
if PATH="$work/commands:$PATH" CUEMOL_TEST_MV="$real_mv" CUEMOL_TEST_DESTINATION="$work/bin/cuemol"   CUEMOL_RELEASE_URL="file://$work/release" CUEMOL_INSTALL_DIR="$work/bin" sh "$root/install.sh"; then
  printf 'Installer accepted a failed binary replacement\n' >&2
  exit 1
fi
test "$("$work/bin/cuemol")" = installed
test "$(cat "$work/bin/cuemol-tools/apbs/bin/apbs")" = previous
rm -rf "$work/bin/cuemol-tools"
if PATH="$work/commands:$PATH" CUEMOL_TEST_MV="$real_mv" CUEMOL_TEST_DESTINATION="$work/bin/cuemol"   CUEMOL_RELEASE_URL="file://$work/release" CUEMOL_INSTALL_DIR="$work/bin" sh "$root/install.sh"; then
  printf 'Installer accepted a failed fresh tool installation\n' >&2
  exit 1
fi
test "$("$work/bin/cuemol")" = installed
test ! -e "$work/bin/cuemol-tools"
CUEMOL_RELEASE_URL="file://$work/release" CUEMOL_INSTALL_DIR="$work/bin" sh "$root/install.sh"
if [ "$#" -gt 0 ]; then
  cp "$1" "$work/bin/cuemol"
  (cd "$work/release" && shasum -a 256 "cuemol-$target.tar.gz" > SHA256SUMS)
  CUEMOL_RELEASE_URL="file://$work/release" "$work/bin/cuemol" self-upgrade
  test "$("$work/bin/cuemol")" = installed
  test "$(cat "$work/bin/cuemol-tools/apbs/bin/apbs")" = bundled
  app="$work/cu emol-rs.app/Contents"
  mkdir -p "$app/MacOS" "$app/Resources/cuemol-tools"
  cat > "$app/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "https://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>cuemol-rs</string>
<key>CFBundleShortVersionString</key><string>1.0.0</string>
<key>CFBundleVersion</key><string>1.0.0</string>
</dict></plist>
PLIST
  cp "$1" "$app/MacOS/cuemol-rs"
  printf 'previous\n' > "$app/Resources/cuemol-tools/marker.txt"
  printf '{"version":"previous"}\n' > "$app/Resources/build-info.json"
  if PATH="$work/commands:$PATH" CUEMOL_TEST_MV="$real_mv" CUEMOL_TEST_DESTINATION="$app/MacOS/cuemol-rs" \
    CUEMOL_RELEASE_URL="file://$work/release" "$app/MacOS/cuemol-rs" self-upgrade; then
    printf 'App update accepted a failed binary replacement\n' >&2
    exit 1
  fi
  cmp "$1" "$app/MacOS/cuemol-rs"
  test "$(cat "$app/Resources/cuemol-tools/marker.txt")" = previous
  test "$(cat "$app/Resources/build-info.json")" = '{"version":"previous"}'
  if [ "$(uname -s)" = Darwin ]; then
    test "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$app/Info.plist")" = 1.0.0
  fi
  CUEMOL_RELEASE_URL="file://$work/release" "$app/MacOS/cuemol-rs" self-upgrade
  cmp "$work/input/cuemol" "$app/MacOS/cuemol-rs"
  test ! -e "$app/MacOS/cuemol"
  test "$("$app/MacOS/cuemol-rs")" = installed
  test "$(cat "$app/Resources/cuemol-tools/apbs/bin/apbs")" = bundled
  test ! -e "$app/Resources/cuemol-tools/marker.txt"
  if [ "$(uname -s)" = Darwin ]; then
    test "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$app/Info.plist")" = 1.2.3
    test "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleVersion' "$app/Info.plist")" = 1.2.3
    cmp "$work/input/build-info.json" "$app/Resources/build-info.json"
  fi
  rm -rf "$app/Resources/cuemol-tools"
  cp "$1" "$app/MacOS/cuemol-rs"
  CUEMOL_RELEASE_URL="file://$work/release" "$app/MacOS/cuemol-rs" self-upgrade
  test "$(cat "$app/Resources/cuemol-tools/apbs/bin/apbs")" = bundled
  test ! -e "$app/MacOS/cuemol-tools"
fi
