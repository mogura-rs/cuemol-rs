#!/bin/sh
set -eu
binary=$1
app=$2
version=$("$binary" --version | sed -E 's/^cuemol v?([0-9]+\.[0-9]+\.[0-9]+).*/\1/')
case "$version" in
  ''|*[!0-9.]*) printf 'Invalid application version: %s\n' "$version" >&2; exit 1 ;;
esac
mkdir -p "$app/Contents/MacOS"
cp "$binary" "$app/Contents/MacOS/cuemol"
cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "https://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleName</key><string>CueMol3</string>
<key>CFBundleDisplayName</key><string>CueMol3</string>
<key>CFBundleIdentifier</key><string>org.cuemol.cuemol3</string>
<key>CFBundleExecutable</key><string>cuemol</string>
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
