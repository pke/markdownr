# Markdownr

## iOS release verification

Before submitting any Markdownr version to App Store Review, run the full iOS end-to-end suite on the newest public iOS Simulator runtime Apple offers for standard iPhones. Fastlane's `ios release` and `ios submit_review` lanes resolve the runtime and stop if the suite fails.

1. Fastlane reads Apple's Software Lookup Service and Xcode Simulator catalog at release time. It uses Apple's preferred build in split SDK groups and selects the newest public standard-iPhone runtime available for the public iOS version or earlier. The lane reports both versions when a public iOS patch has no matching runtime. [Apple Software Lookup Service](https://developer.apple.com/documentation/devicemanagement/deploying-software-updates-using-declarative-management) · [Apple Simulator runtime catalog](https://devimages-cdn.apple.com/downloads/xcode/simulators/index2.dvtdownloadableindex)
2. Fastlane downloads the exact build with `xcodebuild -downloadPlatform iOS -buildVersion <runtime-build>` when needed. It confirms the build and its standard-iPhone support with `xcrun simctl list runtimes -j`. Use the iPhone Duo for explicit foldable-device testing. [Installing Xcode components](https://developer.apple.com/documentation/xcode/downloading-and-installing-additional-xcode-components)
3. To verify locally, run `bundle exec fastlane ios release_tests`. The lane creates a temporary standard iPhone simulator, reruns `expo prebuild --clean` for production, and tests the bundled Release app. It runs all numbered Maestro flows and native XCUITests, including cold and warm Markdown document handoff.
4. Record the app version and build number, Xcode version, simulator model, iOS runtime version and build, and test results. Once the simulator is ready, Fastlane prints the directory containing the build log, Maestro reports and XCTest result bundles.
