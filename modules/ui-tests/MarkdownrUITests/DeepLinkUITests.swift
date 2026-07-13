import XCTest

/// Native coverage for the one thing unit tests can't reach: the **OS actually
/// delivering a `file://` document into the app**. The unit tests mock the read
/// + `openFile`; this drives the real `application(_:open:)` → RN `Linking` →
/// `openDeepLink` path end to end.
///
/// The app registers `net.daringfireball.markdown` with `UISupportsDocumentBrowser`,
/// so its Documents folder is browsable in Files and opening a `.md` there hands
/// the file URL to Markdownr.
final class DeepLinkUITests: XCTestCase {
  private let appId = "dev.dudesoft.markdownr.dev"

  override func setUpWithError() throws {
    continueAfterFailure = false
  }

  /// Sanity gate: the app builds, launches against Metro, and renders its UI.
  /// Asserts the always-present FAB (testID `mainMenuButton`) rather than any
  /// specific content — the app may reopen the last file instead of the welcome
  /// screen. If this fails, the native-test pipeline itself is broken.
  func testAppLaunchesAndRendersUI() throws {
    let app = XCUIApplication()
    app.launch()
    XCTAssertTrue(
      app.wait(for: .runningForeground, timeout: 30),
      "app should reach the foreground"
    )
    let fab = app.descendants(matching: .any)["mainMenuButton"].firstMatch
    XCTAssertTrue(
      fab.waitForExistence(timeout: 90),
      "the FAB menu should render (JS bundle loaded from Metro)"
    )
  }

  /// The real deep link: assert that a `file://` document delivered by the OS
  /// renders in Markdownr. The harness (`run-native-tests.sh`) performs the
  /// actual OS handoff with `simctl openurl file://…/deeplink-fixture.md`
  /// *before* this runs — that is the genuine `application(_:open:)` → RN
  /// `Linking` → `openDeepLink` path. Here we foreground that already-running
  /// instance (activate, not launch, so we don't discard its state) and assert
  /// the fixture's heading is on screen.
  ///
  /// Why not drive the Files app directly? It is too iOS-version-brittle: the
  /// Files bundle id, whether the app folder surfaces, tap-to-rename vs
  /// tap-to-open on a grid label, and Quick Look preview vs hand-off to the
  /// owning app all vary by release. `simctl openurl` reproduces the exact OS
  /// document handoff without any of that fragility.
  func testDeepLinkRendersOpenedFile() throws {
    let app = XCUIApplication(bundleIdentifier: appId)
    app.activate()
    XCTAssertTrue(app.wait(for: .runningForeground, timeout: 20), "app should be foreground")
    XCTAssertTrue(
      app.staticTexts["Deep Link Heading"].waitForExistence(timeout: 30),
      "the openurl-delivered fixture heading should be rendered"
    )
  }
}
