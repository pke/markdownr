#!/usr/bin/env bash
# Run the native XCUITests. The deep-link test needs the OS to hand a file:// URL
# to the app first (XCUITest can't call simctl), so we do the real document
# handoff here — `simctl openurl file://…` → application(_:open:) → RN Linking →
# openDeepLink — then run the bundle, which asserts the fixture rendered.
#
# Prereqs: Metro running on 8092, and the app + test bundle already built
# (`build-for-testing`). Injects the UI-test target if missing. ios/ is
# gitignored; nothing here is committed.
set -euo pipefail

SIM="${SIM:-C77D6539-FB9A-48FD-B287-43DEB25C324A}"
APP_ID="dev.dudesoft.markdownr.dev"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
APP_PATH="$ROOT/ios/build/Build/Products/Debug-iphonesimulator/MarkdownrDev.app"

echo "== ensure app installed + fixture seeded =="
[ -d "$APP_PATH" ] || { echo "build first: xcodebuild build-for-testing …"; exit 1; }
xcrun simctl install "$SIM" "$APP_PATH"
CONTAINER="$(xcrun simctl get_app_container "$SIM" "$APP_ID" data)"
mkdir -p "$CONTAINER/Documents"
cp "$ROOT/modules/ui-tests/fixtures/deeplink-fixture.md" "$CONTAINER/Documents/deeplink-fixture.md"
FIXTURE_URL="file://$CONTAINER/Documents/deeplink-fixture.md"

echo "== real OS handoff: openurl $FIXTURE_URL =="
xcrun simctl terminate "$SIM" "$APP_ID" 2>/dev/null || true
xcrun simctl openurl "$SIM" "$FIXTURE_URL"
# give Metro time to bundle + the app to render on first launch
sleep 12

echo "== run XCUITests (asserts the delivered fixture rendered) =="
cd "$ROOT"
xcodebuild test-without-building \
  -workspace ios/MarkdownrDev.xcworkspace -scheme MarkdownrDev \
  -destination "platform=iOS Simulator,id=$SIM" \
  -derivedDataPath ios/build \
  -only-testing:MarkdownrUITests/DeepLinkUITests \
  CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO
