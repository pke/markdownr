import XCTest

/// Native UI tests for behaviour Maestro can't drive — chiefly the per-row
/// swipe-to-delete gesture on the Recent Files list (react-native-gesture-handler
/// pans need a real continuous touch, which XCUITest produces and Maestro doesn't).
///
/// Recents are seeded by the DEBUG hook in App.tsx via the `-uitestSeedRecents`
/// launch argument (iOS surfaces it through NSUserDefaults / RN Settings).
final class RecentsUITests: XCTestCase {
  override func setUpWithError() throws {
    continueAfterFailure = false
  }

  func testSwipeToDeleteRecentFile() throws {
    let app = XCUIApplication()
    app.launchArguments += ["-uitestSeedRecents", "YES"]
    app.launch()

    // Open the drawer with an edge swipe (react-navigation drawer).
    let start = app.coordinate(withNormalizedOffset: CGVector(dx: 0.01, dy: 0.5))
    let end = app.coordinate(withNormalizedOffset: CGVector(dx: 0.9, dy: 0.5))
    start.press(forDuration: 0.1, thenDragTo: end)

    // Open the full recents list.
    let showAll = app.staticTexts.matching(
      NSPredicate(format: "label BEGINSWITH 'Show All'")
    ).firstMatch
    XCTAssertTrue(showAll.waitForExistence(timeout: 15), "Show All should appear")
    showAll.tap()

    // Swipe-delete the Alpha row (the gesture Maestro falls through as a tap).
    let alpha = app.staticTexts["Alpha"]
    XCTAssertTrue(alpha.waitForExistence(timeout: 10), "Alpha recent should be listed")
    alpha.swipeLeft()

    let deleteAction = app.staticTexts["Delete"]
    XCTAssertTrue(deleteAction.waitForExistence(timeout: 5), "Delete action should be revealed")
    deleteAction.tap()

    // Alpha is gone; the others remain.
    XCTAssertFalse(alpha.waitForExistence(timeout: 3), "Alpha should be deleted")
    XCTAssertTrue(app.staticTexts["Beta"].exists, "Beta should remain")
  }
}
