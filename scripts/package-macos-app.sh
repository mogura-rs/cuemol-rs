#!/bin/sh
set -eu
binary=$1
app=$2
mkdir -p "$app/Contents/MacOS"
cp "$binary" "$app/Contents/MacOS/cuemol"
cat > "$app/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "https://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleName</key><string>CueMol</string>
<key>CFBundleDisplayName</key><string>CueMol</string>
<key>CFBundleIdentifier</key><string>org.cuemol.cuemol3</string>
<key>CFBundleExecutable</key><string>cuemol</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleVersion</key><string>1</string>
<key>NSHighResolutionCapable</key><true/>
</dict></plist>
PLIST
