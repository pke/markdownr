# Native UI tests (XCUITest)

Native coverage for app launch, iOS Markdown document handoff, and the
swipe-to-delete gesture in Recent Files.

## Release verification

Run the release checks locally with Fastlane:

```sh
bundle exec fastlane ios release_tests
```

The lane requires Xcode, Python 3, and Maestro. It resolves the latest public
standard-iPhone runtime from Apple and installs the exact build if needed.
See [AGENTS.md](../../AGENTS.md) for the Apple sources. The lane regenerates
`ios/` with the production Expo configuration and builds a bundled Release app
on a temporary iPhone simulator. It runs all numbered Maestro
flows and the XCUITests. The release harness opens real Markdown files to seed
the recents tests. Fastlane prints the path to the build log, Maestro reports,
and XCTest results. Both `ios release` and `ios submit_review` require this run.
An optional `runtime_build` argument asserts that Apple's current selection
matches the supplied build.

## What's here

- **`MarkdownrUITests/DeepLinkUITests.swift`** verifies that the app renders
  on launch and displays Markdown files opened from iOS with the app stopped
  and already running.
- **`MarkdownrUITests/RecentsUITests.swift`** verifies that swiping to delete a
  single recent file leaves the other entries in place.
- **`inject-target.rb`** — adds the XCUITest target to the generated `ios/`
  project using the **`xcodeproj` Ruby gem**. The Expo plugin calls this script
  when prebuild runs with `WITH_UITESTS=1`.
- **`run-native-tests.sh`** — the harness. XCUITest can't call `simctl`, so it
  performs the real document handoff itself (`simctl openurl file://…`) and then
  runs the bundle, which asserts the fixture rendered.
- **`probe-delivery.sh`** — one-off diagnostic used to confirm `simctl openurl`
  delivers to the app (it does) and screenshot the result.
- **`fixtures/deeplink-fixture.md`** — the document opened via deep link.

## Why `openurl`, not driving the Files app

Driving the real Files app in XCUITest is too iOS-version-brittle — every step is
a moving target: the Files bundle id (`com.apple.DocumentsApp` on iOS 26, not
`com.apple.DocumentsUI`), whether the app's Documents folder surfaces, its
display name (`Markdownr Dev` with a space, not the target name), tap-to-rename
vs tap-to-open on a grid label, and Quick Look preview vs hand-off to the owning
app. `simctl openurl file://…` reproduces the exact OS `application(_:open:)`
handoff without any of that fragility, and was verified to render the fixture
end to end (see `probe-delivery.sh`).

## Running the development launch and document tests

```sh
# 1. Metro (custom port)
APP_VARIANT=development npx expo start --port 8092

# 2. regenerate iOS with the native test target
APP_VARIANT=development WITH_UITESTS=1 npx expo prebuild --clean -p ios

# 3. build once, then run the harness
xcodebuild build-for-testing -workspace ios/MarkdownrDev.xcworkspace \
  -scheme MarkdownrDev -destination 'platform=iOS Simulator,name=iPhone 17 Pro' \
  -derivedDataPath ios/build CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO
bash modules/ui-tests/run-native-tests.sh
```
