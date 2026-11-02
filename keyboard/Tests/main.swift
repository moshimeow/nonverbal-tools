import Foundation

func check(_ condition: @autoclosure () -> Bool, _ message: String) {
    guard condition() else { fatalError(message) }
}

// Every English letter must be reachable exactly once, with its printed swipe.
let alphabet = KeyMap.keys.flatMap { key in
    Direction.allCases.compactMap { key.text($0) }.filter {
        $0.count == 1 && $0 >= "a" && $0 <= "z"
    }
}.sorted().joined()
check(alphabet == "abcdefghijklmnopqrstuvwxyz", "Missing or duplicated letter: \(alphabet)")
for (index, key) in KeyMap.keys.enumerated() {
    check(key.text(.center) == String(index), "Digit taps must insert digits")
}
func swipe(_ x: CGFloat, _ y: CGFloat) -> Direction {
    var selection = SwipeSelection()
    selection.update(dx: x, dy: y)
    return selection.direction
}
check(swipe(0, 0) == .center, "Tap is a digit")
check(swipe(7, 2) == .center, "Tremor must not select a letter")
check(swipe(-30, 2) == .left, "Left swipe")
check(swipe(2, -30) == .up, "Up swipe")
check(swipe(30, 2) == .right, "Right swipe")
check(swipe(2, 30) == .down, "Down swipe")
check(swipe(-150, 2) == .left, "Long swipes retain the initial key")
var selection = SwipeSelection()
selection.update(dx: -20, dy: 0)
selection.update(dx: -11, dy: 0)
check(selection.direction == .left, "Threshold hysteresis")
selection.update(dx: -4, dy: 0)
check(selection.direction == .center, "Returning to origin restores digit")
selection.update(dx: 20, dy: 0)
selection.update(dx: 20, dy: -21)
check(selection.direction == .right, "Diagonal jitter must not change selection")
selection.update(dx: 20, dy: -28)
check(selection.direction == .up, "Intentional direction changes must work")
check(KeyMap.keys[4].text(.down) == nil, "Unassigned gestures must not insert digits")
let hello: [(Int, Direction)] = [(4, .up), (3, .up), (5, .right), (5, .right), (6, .right)]
check(hello.compactMap { KeyMap.keys[$0.0].text($0.1) }.joined() == "hello", "Reference gesture sequence")
check(KeyMap.keys[7].text(.down) == "s" && KeyMap.keys[9].text(.down) == "z", "Four-letter keys")
check(KeyMap.symbolRows.count == 4 && KeyMap.symbolRows.allSatisfy { $0.count == 13 }, "Reference symbol grid dimensions")
let symbols = KeyMap.symbolRows.flatMap { $0 }
for symbol in ["?", "!", "(", ")", "[", "]", "{", "}", "@", "#", "/", "\\", "\"", "'", "£", "€", "$", "…"] {
    check(symbols.contains(symbol), "Missing symbol: \(symbol)")
}
print("PASS: alphabet, digits, swipe directions, dead zone, hysteresis, long swipes, return to origin, unassigned gestures, hello, symbol coverage")
