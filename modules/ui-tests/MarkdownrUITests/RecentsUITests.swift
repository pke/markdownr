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

    openRecentFiles(in: app)

    let rows = app.scrollViews.descendants(matching: .any).matching(identifier: "recentFileRow")
    let alpha = rows.matching(NSPredicate(format: "label CONTAINS %@", "Alpha")).firstMatch
    let beta = rows.matching(NSPredicate(format: "label CONTAINS %@", "Beta")).firstMatch
    let gamma = rows.matching(NSPredicate(format: "label CONTAINS %@", "Gamma")).firstMatch
    XCTAssertTrue(alpha.waitForExistence(timeout: 10), "Alpha recent should be listed")
    let swipeStart = alpha.coordinate(withNormalizedOffset: CGVector(dx: 0.90, dy: 0.5))
    let shortSwipeEnd = swipeStart.withOffset(CGVector(dx: -24, dy: 0))
    swipeStart.press(forDuration: 0.1, thenDragTo: shortSwipeEnd, withVelocity: .slow, thenHoldForDuration: 0.2)
    XCTAssertTrue(alpha.exists, "Releasing a short swipe should leave Recent Files open")

    swipeAcrossRow(alpha, from: 0.95, to: 0.04)
    XCTAssertFalse(alpha.waitForExistence(timeout: 3), "A full swipe should remove Alpha immediately")
    XCTAssertTrue(beta.exists, "Beta should remain")
    XCTAssertTrue(gamma.exists, "Gamma should remain")

    let done = app.descendants(matching: .any).matching(identifier: "Done").firstMatch
    done.tap()
    openRecentFiles(in: app)
    XCTAssertTrue(beta.waitForExistence(timeout: 10), "Manage should open the list with two entries")
    try revealAndTapRemove(beta, in: app)
    XCTAssertFalse(beta.waitForExistence(timeout: 3), "Beta should be deletable with two entries")

    done.tap()
    openRecentFiles(in: app)
    XCTAssertTrue(gamma.waitForExistence(timeout: 10), "Manage should open the list with one entry")
    gamma.tap()
    XCTAssertFalse(gamma.waitForExistence(timeout: 3), "Tapping a row should close Recent Files")
    XCTAssertTrue(app.staticTexts["Gamma"].waitForExistence(timeout: 10), "A tap should open Gamma")
  }

  private func openRecentFiles(in app: XCUIApplication) {
    let start = app.coordinate(withNormalizedOffset: CGVector(dx: 0.01, dy: 0.5))
    let end = app.coordinate(withNormalizedOffset: CGVector(dx: 0.9, dy: 0.5))
    start.press(forDuration: 0.1, thenDragTo: end)

    let manage = app.buttons["Manage"]
    XCTAssertTrue(manage.waitForExistence(timeout: 15), "Manage should appear in the drawer")
    manage.tap()

    let removeActions = app.buttons.matching(identifier: "recentFileRemove")
    let previewVisible = NSPredicate { _, _ in
      removeActions.allElementsBoundByIndex.contains(where: \.isHittable)
    }
    let shown = XCTNSPredicateExpectation(predicate: previewVisible, object: nil)
    XCTAssertEqual(XCTWaiter.wait(for: [shown], timeout: 5), .completed, "Opening the sheet should preview Remove")
    let closed = XCTNSPredicateExpectation(predicate: NSCompoundPredicate(notPredicateWithSubpredicate: previewVisible), object: nil)
    XCTAssertEqual(XCTWaiter.wait(for: [closed], timeout: 5), .completed, "The preview should close on its own")
  }

  private func swipeAcrossRow(_ row: XCUIElement, from start: CGFloat, to end: CGFloat, velocity: XCUIGestureVelocity = .slow) {
    let swipeStart = row.coordinate(withNormalizedOffset: CGVector(dx: start, dy: 0.5))
    let swipeEnd = row.coordinate(withNormalizedOffset: CGVector(dx: end, dy: 0.5))
    swipeStart.press(forDuration: 0.1, thenDragTo: swipeEnd, withVelocity: velocity, thenHoldForDuration: 0.1)
  }

  private func revealAndTapRemove(_ row: XCUIElement, in app: XCUIApplication) throws {
    swipeAcrossRow(row, from: 0.9, to: 0.52, velocity: .fast)
    XCTAssertTrue(row.exists, "Releasing a shorter swipe should leave the entry in the list")
    let removeActions = app.buttons.matching(identifier: "recentFileRemove")
    XCTAssertTrue(removeActions.firstMatch.waitForExistence(timeout: 5), "Remove action should be revealed")
    let removeAction = try XCTUnwrap(
      removeActions.allElementsBoundByIndex.first(where: \.isHittable),
      "A Remove action should be tappable"
    )
    XCTAssertEqual(removeAction.label, "Remove")
    removeAction.tap()
  }
}
