# Native UI tests (XCUITest)

Covers behaviour Maestro can't drive — chiefly the **per-row swipe-to-delete**
gesture on the Recent Files list. `react-native-gesture-handler`'s pan needs a
real continuous touch; XCUITest's `swipeLeft()` produces one, Maestro's synthetic
swipe falls through to the row's tap and opens the file instead.

## What's here

- **`MarkdownrUITests/RecentsUITests.swift`** — the test: launch with recents
  seeded, open the drawer, Show All, swipe-delete the "Alpha" row, assert it's
  gone and the others remain. This is complete and correct.
- **`plugin.js`** — an Expo config plugin that adds the XCUITest target to the
  generated Xcode project so it survives `expo prebuild --clean`. **Work in
  progress — not registered in `app.config.js`** (see Status).

## Seeding

Recents can't be created through the UI without the native picker, so the
DEBUG-only hook in `App.tsx` seeds three files (Alpha/Beta/Gamma) when the
`-uitestSeedRecents YES` launch argument is present. iOS surfaces launch
arguments through `NSUserDefaults`, which RN's `Settings` module reads — no
native bridge needed. The test sets that argument via `app.launchArguments`.

## Status

The XCUITest and the seed hook are done. The **config plugin is not finished**:
`node-xcode` has no first-class UI-test target type, and the target this plugin
produces currently fails Expo's xcodeproj serializer with
`Invalid target: undefined`. Registering the plugin **breaks `expo prebuild`**,
so it is intentionally left out of `app.config.js`.

### To run the test today (manual target)

1. `APP_VARIANT=development npx expo prebuild -p ios`
2. In Xcode, add a **UI Testing Bundle** target named `MarkdownrUITests`
   (host application = the app target), and add `RecentsUITests.swift` to it.
3. Start Metro (`APP_VARIANT=development npx expo start --port 8092`).
4. `xcodebuild test -workspace ios/MarkdownrDev.xcworkspace -scheme MarkdownrDev \
     -destination 'platform=iOS Simulator,name=iPhone 17 Pro' \
     -only-testing:MarkdownrUITests`

### To finish the plugin

The remaining work is producing a valid UI-test `PBXNativeTarget` (product type
`com.apple.product-type.bundle.ui-testing`, `TEST_TARGET_NAME` = app target, a
Sources phase with the Swift file, and a target dependency) that Expo's
serializer accepts. `@config-plugins/detox` is a working reference for adding a
UI-test target to an Expo project. Once it prebuilds cleanly, register it in
`app.config.js` and wire a `test` action into the app scheme.
