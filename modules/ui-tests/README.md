# Native UI tests (XCUITest)

Native coverage for the one thing unit tests can't reach: the **OS actually
delivering a `file://` document into the app** (`application(_:open:)` → RN
`Linking` → `openDeepLink`). The unit tests mock the read + `openFile`.

**Status: both tests pass** on the iPhone 17 Pro simulator (iOS 26.5).

## What's here

- **`MarkdownrUITests/DeepLinkUITests.swift`** — two tests:
  - `testAppLaunchesAndRendersUI` — sanity: app builds, launches against Metro,
    renders (asserts the always-present FAB, testID `mainMenuButton`).
  - `testDeepLinkRendersOpenedFile` — asserts an OS-delivered `file://` document
    renders in Markdownr.
- **`inject-target.rb`** — adds the XCUITest target to the generated `ios/`
  project using the **`xcodeproj` Ruby gem** (CocoaPods ships it). This replaces
  the abandoned `plugin.js` — node-xcode has no `ui-testing` target type, so the
  config-plugin route failed the Expo serializer (`Invalid target: undefined`).
  The gem creates a proper `ui_test_bundle`; the one gotcha is `PRODUCT_NAME`
  (unset → `-Runner.app`/`.xctest` name collision), which the script fixes.
- **`run-native-tests.sh`** — the harness. XCUITest can't call `simctl`, so it
  performs the real document handoff itself (`simctl openurl file://…`) and then
  runs the bundle, which asserts the fixture rendered.
- **`probe-delivery.sh`** — one-off diagnostic used to confirm `simctl openurl`
  delivers to the app (it does) and screenshot the result.
- **`fixtures/deeplink-fixture.md`** — the document opened via deep link.
- **`plugin.js`** — the abandoned node-xcode approach, kept for reference. Not
  registered in `app.config.js` (would break `expo prebuild`).

## Why `openurl`, not driving the Files app

Driving the real Files app in XCUITest is too iOS-version-brittle — every step is
a moving target: the Files bundle id (`com.apple.DocumentsApp` on iOS 26, not
`com.apple.DocumentsUI`), whether the app's Documents folder surfaces, its
display name (`Markdownr Dev` with a space, not the target name), tap-to-rename
vs tap-to-open on a grid label, and Quick Look preview vs hand-off to the owning
app. `simctl openurl file://…` reproduces the exact OS `application(_:open:)`
handoff without any of that fragility, and was verified to render the fixture
end to end (see `probe-delivery.sh`).

## Running

```sh
# 1. Metro (custom port)
APP_VARIANT=development npx expo start --port 8092

# 2. prebuild if ios/ is stale, then inject the test target
APP_VARIANT=development npx expo prebuild -p ios
ruby modules/ui-tests/inject-target.rb          # needs the xcodeproj gem

# 3. build once, then run the harness
xcodebuild build-for-testing -workspace ios/MarkdownrDev.xcworkspace \
  -scheme MarkdownrDev -destination 'platform=iOS Simulator,name=iPhone 17 Pro' \
  -derivedDataPath ios/build CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO
bash modules/ui-tests/run-native-tests.sh
```

## Caveats / not-yet-done

- The AppDelegate dev build needs the Metro host pinned (`provider.jsLocation =
  "localhost:8092"`) — `packagerServerHost()` returns nil for the dev variant on
  iOS 26. `inject-target.rb` does **not** patch AppDelegate; do it by hand after
  prebuild (or fold it into the config plugin if this is ever productionized).
- The target injection is a local step, not wired into `expo prebuild`. To make
  it survive `--clean`, finish a config plugin (the `xcodeproj`-gem logic here is
  the working reference; `@config-plugins/detox` shows the plugin shape).
- `RecentsUITests.swift` (swipe-to-delete) is separate and still needs its seed +
  selectors verified against iOS 26.
