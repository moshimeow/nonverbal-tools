import Foundation

enum Direction: String, CaseIterable {
    case center, left, up, right, down
}

struct KeySpec {
    let digit: String
    let left: String
    let up: String
    let right: String
    let down: String

    func text(_ direction: Direction) -> String? {
        let value: String
        switch direction {
        case .center: value = digit
        case .left: value = left
        case .up: value = up
        case .right: value = right
        case .down: value = down
        }
        return value.isEmpty ? nil : value
    }
}

enum KeyMap {
    // Symbols visible in the video. The lower-left shift is adapted to Delete;
    // the fixed 2ABC layout has no alternate language or symbol alphabet.
    static let symbolRows: [[String]] = [
        ["£", "¥", "€", "$", "₹", "^", "&", "*", "(", ")", "№", "√", "÷"],
        ["~", "`", "{", "}", "%", "_", "-", "=", "|", "+", "§", "∷", "‡"],
        ["@", "[", "]", "#", "/", "\\", "'", "\"", "«", "»", "—", "‑", "–"],
        ["⌫", "…", "<", ">", "!", ";", ":", "?", "‹", "›", "±", ".", ","]
    ]

    // Transcribed from the front page of the supplied video, before 02:26.
    static let keys: [KeySpec] = [
        .init(digit: "0", left: "", up: "", right: "", down: ""),
        .init(digit: "1", left: "'", up: "@", right: "\"", down: "*"),
        .init(digit: "2", left: "a", up: "b", right: "c", down: "æ"),
        .init(digit: "3", left: "d", up: "e", right: "f", down: "#"),
        .init(digit: "4", left: "g", up: "h", right: "i", down: ""),
        .init(digit: "5", left: "j", up: "k", right: "l", down: "÷"),
        .init(digit: "6", left: "m", up: "n", right: "o", down: "œ"),
        .init(digit: "7", left: "p", up: "q", right: "r", down: "s"),
        .init(digit: "8", left: "t", up: "u", right: "v", down: "~"),
        .init(digit: "9", left: "w", up: "x", right: "y", down: "z")
    ]
}

struct SwipeSelection {
    private(set) var direction = Direction.center
    static let threshold: CGFloat = 12
    static let hysteresis: CGFloat = 3

    mutating func update(dx: CGFloat, dy: CGFloat) {
        let distance = hypot(dx, dy)
        // Returning near the touch origin restores the digit. Small tremors at
        // the boundary retain the current choice instead of buzzing repeatedly.
        if distance < Self.threshold - Self.hysteresis {
            direction = .center
            return
        }
        if direction == .center && distance < Self.threshold { return }
        let candidate: Direction = abs(dx) > abs(dy)
            ? (dx < 0 ? .left : .right) : (dy < 0 ? .up : .down)
        if direction != .center && candidate != direction &&
            abs(abs(dx) - abs(dy)) < Self.hysteresis { return }
        direction = candidate
    }
}
