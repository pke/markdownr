#!/bin/sh
# Wraps the macOS binary into Markdownr.app: Info.plist (opens Markdown files from Finder),
# the icon from assets/markdownr-256.png, ad hoc signature.
#   sh scripts/mac-bundle.sh target/aarch64-apple-darwin/release/markdownr out/Markdownr.app
set -e
BIN=$1
APP=$2
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BIN" "$APP/Contents/MacOS/Markdownr"
ICONSET=$(mktemp -d)/AppIcon.iconset
mkdir -p "$ICONSET"
for s in 16 32 128 256; do
    sips -z $s $s assets/markdownr-256.png --out "$ICONSET/icon_${s}x${s}.png" >/dev/null
done
for s in 16 32 128; do
    sips -z $((s * 2)) $((s * 2)) assets/markdownr-256.png --out "$ICONSET/icon_${s}x${s}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/AppIcon.icns"
cat > "$APP/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleName</key><string>Markdownr</string>
<key>CFBundleIdentifier</key><string>app.dudesoft.markdownr</string>
<key>CFBundleExecutable</key><string>Markdownr</string>
<key>CFBundleIconFile</key><string>AppIcon</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>0.1</string>
<key>LSMinimumSystemVersion</key><string>12.0</string>
<key>NSHighResolutionCapable</key><true/>
<key>NSPrincipalClass</key><string>NSApplication</string>
<key>CFBundleDocumentTypes</key><array><dict>
  <key>CFBundleTypeName</key><string>Markdown</string>
  <key>CFBundleTypeRole</key><string>Viewer</string>
  <key>LSItemContentTypes</key><array><string>net.daringfireball.markdown</string><string>public.plain-text</string></array>
  <key>CFBundleTypeExtensions</key><array><string>md</string><string>markdown</string><string>mdown</string><string>mkd</string></array>
</dict></array>
</dict></plist>
PLIST
codesign --force --sign - "$APP" 2>/dev/null || true
ls -l "$APP/Contents/MacOS/Markdownr"
