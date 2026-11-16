#!/usr/bin/env bash
# Empirically determine how a file:// deep link reaches the app on the sim.
# Installs the freshly built app, seeds a fixture into its Documents, then tries
# `simctl openurl` and screenshots the result so we can see whether the app
# opened the file or iOS intercepted it with a Save/import sheet.
set -euo pipefail

SIM="C77D6539-FB9A-48FD-B287-43DEB25C324A"
APP_ID="dev.dudesoft.markdownr.dev"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
APP_PATH="$ROOT/ios/build/Build/Products/Debug-iphonesimulator/MarkdownrDev.app"
SHOTS="/private/tmp/claude-501/-Users-pkursawe-projects-private-markdownr/387a248f-6175-4b01-a1e1-cbab67a9855d/scratchpad"

echo "== install =="
[ -d "$APP_PATH" ] || { echo "no .app at $APP_PATH"; exit 1; }
xcrun simctl install "$SIM" "$APP_PATH"

echo "== seed fixture into app Documents =="
CONTAINER="$(xcrun simctl get_app_container "$SIM" "$APP_ID" data)"
mkdir -p "$CONTAINER/Documents"
cp "$ROOT/modules/ui-tests/fixtures/deeplink-fixture.md" "$CONTAINER/Documents/deeplink-fixture.md"
FIXTURE_URL="file://$CONTAINER/Documents/deeplink-fixture.md"
echo "fixture: $FIXTURE_URL"

echo "== attempt A: simctl openurl (does it deliver to the app?) =="
xcrun simctl terminate "$SIM" "$APP_ID" 2>/dev/null || true
set +e
xcrun simctl openurl "$SIM" "$FIXTURE_URL"
echo "openurl exit=$?"
set -e
sleep 4
xcrun simctl io "$SIM" screenshot "$SHOTS/probe-openurl.png"
echo "screenshot -> probe-openurl.png"

echo "== also list what Files shows (for the self-contained XCUITest path) =="
xcrun simctl launch "$SIM" com.apple.DocumentsUI 2>/dev/null || true
sleep 3
xcrun simctl io "$SIM" screenshot "$SHOTS/probe-files.png"
echo "screenshot -> probe-files.png"
