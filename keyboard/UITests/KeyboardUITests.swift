import XCTest

final class KeyboardUITests: XCTestCase {
    private var app: XCUIApplication!
    override func setUpWithError() throws {
        continueAfterFailure = false
        app = XCUIApplication()
        app.launch()
        XCTAssertTrue(app.textViews["practice-editor"].waitForExistence(timeout: 10))
        XCTAssertTrue(key("2").waitForExistence(timeout: 10))
    }
    private var editor: XCUIElement { app.textViews["practice-editor"] }
    private func key(_ digit: String) -> XCUIElement { app.descendants(matching: .any)["key-\(digit)"] }
    private func flick(_ digit: String, _ dx: CGFloat, _ dy: CGFloat) {
        let origin = key(digit).coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5))
        origin.press(forDuration: 0.04, thenDragTo: origin.withOffset(CGVector(dx: dx, dy: dy)), withVelocity: .fast, thenHoldForDuration: 0)
    }
    func testLetterSwipesNumbersAndEditing() {
        flick("4", 0, -30)
        flick("3", 0, -30)
        flick("5", 30, 0)
        flick("5", 30, 0)
        flick("6", 30, 0)
        XCTAssertEqual(editor.value as? String, "hello")
        app.buttons["key-space"].tap()
        key("2").tap()
        flick("7", 0, 30)
        flick("9", 0, 30)
        XCTAssertEqual(editor.value as? String, "hello 2sz")
        app.buttons["key-delete"].tap()
        XCTAssertEqual(editor.value as? String, "hello 2s")
        app.buttons["key-left"].tap()
        flick("2", -30, 0)
        XCTAssertEqual(editor.value as? String, "hello 2as")
    }
    func testShiftIsOneShotAndSpaceDoesNotSwitchLayouts() {
        app.buttons["key-shift"].tap()
        flick("2", -30, 0)
        flick("2", 0, -30)
        XCTAssertEqual(editor.value as? String, "Ab")
        app.buttons["key-space"].press(forDuration: 1)
        XCTAssertEqual(editor.value as? String, "Ab ")
        flick("2", 30, 0)
        XCTAssertEqual(editor.value as? String, "Ab c")
    }
    func testLongSwipeStaysOnStartingKeyAndUnusedDirectionIsEmpty() {
        flick("2", 120, 0)
        XCTAssertEqual(editor.value as? String, "c")
        flick("4", 0, 30)
        XCTAssertEqual(editor.value as? String, "c")
    }

    func testDeleteRepeatStopsWhenFingerLifts() {
        for _ in 0..<8 { key("2").tap() }
        app.buttons["key-delete"].press(forDuration: 0.9)
        let remaining = editor.value as? String ?? ""
        XCTAssertLessThan(remaining.count, 8)
        flick("2", -30, 0)
        app.buttons["key-space"].tap()
        app.buttons["key-space"].tap()
        XCTAssertEqual(editor.value as? String, remaining + "a  ")
    }

    func testSymbolPageAndReturnToLetters() {
        app.buttons["key-symbols"].tap()
        app.descendants(matching: .any)["symbol-8"].tap()
        app.descendants(matching: .any)["symbol-9"].tap()
        app.descendants(matching: .any)["symbol-46"].tap()
        XCTAssertEqual(editor.value as? String, "()?")
        app.descendants(matching: .any)["symbol-39"].tap()
        XCTAssertEqual(editor.value as? String, "()")
        app.buttons["key-space"].press(forDuration: 1)
        XCTAssertEqual(editor.value as? String, "() ")
        XCTAssertTrue(app.descendants(matching: .any)["symbol-8"].exists)
        app.buttons["key-symbols"].tap()
        flick("2", -30, 0)
        XCTAssertEqual(editor.value as? String, "() a")
    }

    func testDragFromPeriodSelectsSymbolAndReturnsToLetters() {
        let period = app.buttons["key-period"]
        let start = period.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5))
        // The symbol grid spans the same three number rows. Its third row,
        // fourth column is # in the reference; derive the target from key frames.
        let gridTop = key("1").frame.minY
        let gridBottom = key("7").frame.maxY
        let target = app.coordinate(withNormalizedOffset: .zero).withOffset(CGVector(
            dx: app.frame.width / 13 * 3.5,
            dy: gridTop + (gridBottom - gridTop) / 4 * 2.5))
        start.press(forDuration: 0.04, thenDragTo: target, withVelocity: .slow, thenHoldForDuration: 0)
        XCTAssertEqual(editor.value as? String, "#")
        XCTAssertTrue(key("2").exists)
        period.tap()
        XCTAssertEqual(editor.value as? String, "#.")
    }
}

final class SystemKeyboardUITests: XCTestCase {
    func testEnableAndTypeWithExtension() {
        continueAfterFailure = false
        let app = XCUIApplication()
        app.launch()
        XCTAssertTrue(app.textViews["practice-editor"].waitForExistence(timeout: 10))
        let settings = XCUIApplication(bundleIdentifier: "com.apple.Preferences")
        settings.launch()
        for _ in 0..<5 {
            if settings.navigationBars["Settings"].exists { break }
            let back = settings.navigationBars.buttons.firstMatch
            if back.exists { back.tap() }
        }
        func cell(_ title: String) -> XCUIElement {
            settings.cells.matching(NSPredicate(format: "label BEGINSWITH %@", title)).firstMatch
        }
        func tap(_ title: String) {
            for _ in 0..<5 {
                let contentTop = settings.navigationBars.firstMatch.frame.maxY
                let prefix = title.replacingOccurrences(of: "…", with: "")
                for row in settings.cells.matching(NSPredicate(format: "label BEGINSWITH %@", prefix)).allElementsBoundByIndex {
                    if row.frame.midY.isFinite && row.frame.minY >= contentTop && row.frame.height > 0 && row.isHittable {
                        row.tap()
                        return
                    }
                }
                for text in settings.staticTexts.matching(identifier: title).allElementsBoundByIndex {
                    // Settings sometimes exposes a frameless outer cell and a
                    // navigation title with the same text. Tap the visible row.
                    if text.frame.midY.isFinite && text.frame.minY >= contentTop && text.isHittable {
                        text.tap()
                        return
                    }
                }
                settings.swipeUp()
            }
            XCTFail("Missing Settings row: \(title)\n\(settings.debugDescription)")
        }
        tap("General")
        tap("Keyboard")
        tap("Keyboards")
        if !cell("2ABC").exists {
            tap("Add New Keyboard…")
            tap("2ABC Practice")
        }
        XCTAssertTrue(cell("2ABC").waitForExistence(timeout: 10), settings.debugDescription)
        app.activate()
        app.buttons["practice-keyboard"].tap()
        let letterKey = app.descendants(matching: .any)["key-2"]
        if !letterKey.exists {
            let globe = app.buttons.matching(NSPredicate(format: "label CONTAINS[c] 'next keyboard'")).firstMatch
            XCTAssertTrue(globe.waitForExistence(timeout: 5), app.debugDescription)
            globe.press(forDuration: 1)
            // A short globe tap may only alternate the two most recently used
            // keyboards. Explicitly select the new extension from the picker.
            let options = app.descendants(matching: .any).matching(NSPredicate(format: "label BEGINSWITH '2ABC'")).allElementsBoundByIndex
            var chosen = false
            for option in options where [.button, .cell, .staticText].contains(option.elementType) {
                if option.frame.midY.isFinite && option.frame.minY > 120 && option.isHittable {
                    option.tap()
                    chosen = true
                    break
                }
            }
            XCTAssertTrue(chosen, app.debugDescription)
        }
        XCTAssertTrue(letterKey.waitForExistence(timeout: 10), app.debugDescription)
        let nine = app.descendants(matching: .any)["key-9"]
        let untouched = nine.screenshot()
        nine.tap()
        let redrawn = nine.screenshot()
        XCTAssertEqual(untouched.pngRepresentation, redrawn.pngRepresentation,
                       "Initial key glyphs must match their appearance after interaction")
        app.buttons["key-delete"].tap()
        let start = letterKey.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5))
        start.press(forDuration: 0.04, thenDragTo: start.withOffset(CGVector(dx: -30, dy: 0)), withVelocity: .fast, thenHoldForDuration: 0)
        XCTAssertEqual(app.textViews["practice-editor"].value as? String, "a")
        app.buttons["key-space"].press(forDuration: 1)
        XCTAssertEqual(app.textViews["practice-editor"].value as? String, "a ")
        XCTAssertTrue(letterKey.exists, "Holding space must leave 2ABC selected")
        app.buttons["key-symbols"].tap()
        app.descendants(matching: .any)["symbol-46"].tap()
        XCTAssertEqual(app.textViews["practice-editor"].value as? String, "a ?")
        app.buttons["key-symbols"].tap()
        app.buttons["key-globe"].tap()
        XCTAssertTrue(letterKey.waitForNonExistence(timeout: 5), "The globe must switch away from 2ABC")
    }
}
