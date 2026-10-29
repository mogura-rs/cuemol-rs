#!/bin/sh
set -eu
binary=$1
app=$2
version=$("$binary" --version | sed -E 's/^cuemol(-rs)? v?([0-9]+\.[0-9]+\.[0-9]+).*/\2/')
case "$version" in
  ''|*[!0-9.]*) printf 'Invalid application version: %s\n' "$version" >&2; exit 1 ;;
esac
mkdir -p "$app/Contents/MacOS"
cp "$binary" "$app/Contents/MacOS/cuemol-rs"
cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "https://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleName</key><string>cuemol-rs</string>
<key>CFBundleDisplayName</key><string>cuemol-rs</string>
<key>CFBundleIdentifier</key><string>org.cuemol.cuemol3</string>
<key>CFBundleExecutable</key><string>cuemol-rs</string>
<key>CFBundleIconFile</key><string>cuemol-rs</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>$version</string>
<key>CFBundleVersion</key><string>$version</string>
<key>NSHighResolutionCapable</key><true/>
</dict></plist>
PLIST

if [ "$#" -ge 3 ]; then
  mkdir -p "$app/Contents/Resources"
  cp -R "$3" "$app/Contents/Resources/cuemol-tools"
fi

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
icon_work=$(mktemp -d "${TMPDIR:-/tmp}/cuemol-icon.XXXXXX")
trap 'rm -rf "$icon_work"' EXIT HUP INT TERM
iconset="$icon_work/cuemol-rs.iconset"
mkdir -p "$iconset" "$app/Contents/Resources"
for size in 16 32 128 256 512; do
  sips -s format png -z "$size" "$size" "$script_dir/../cuemol-rs.png" --out "$iconset/icon_${size}x${size}.png" >/dev/null
  double=$((size * 2))
  sips -s format png -z "$double" "$double" "$script_dir/../cuemol-rs.png" --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$iconset" -o "$app/Contents/Resources/cuemol-rs.icns"
