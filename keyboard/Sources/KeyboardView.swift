import UIKit

private final class KeyboardButton: UIButton {
    var activateAccessibly: (() -> Void)?
    override func accessibilityActivate() -> Bool {
        guard let activateAccessibly else { return super.accessibilityActivate() }
        activateAccessibly()
        return true
    }
}

final class SwipeKey: UIControl {
    let spec: KeySpec
    var uppercase = false { didSet { setNeedsDisplay() } }
    var onPreview: ((String?) -> Void)?
    var onCommit: ((String) -> Void)?
    var onSelection: (() -> Void)?
    private var origin = CGPoint.zero
    private var selection = SwipeSelection()
    private var trackingTouch = false

    init(_ spec: KeySpec) {
        self.spec = spec
        super.init(frame: .zero)
        // Extension presentation resizes these views after their first draw.
        // Redraw the glyphs instead of stretching the old backing bitmap.
        contentMode = .redraw
        isOpaque = false
        isExclusiveTouch = true
        isAccessibilityElement = true
        accessibilityLabel = "\(spec.digit)"
        accessibilityHint = "Tap for the number. Swipe toward a letter."
        accessibilityTraits = .keyboardKey
        accessibilityCustomActions = Direction.allCases.filter { $0 != .center }.compactMap { direction in
            guard let text = spec.text(direction) else { return nil }
            return UIAccessibilityCustomAction(name: "Type \(text), \(direction.rawValue)") { [weak self] _ in
                guard let self else { return false }
                self.onCommit?(self.cased(text))
                return true
            }
        }
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

    private func cased(_ text: String) -> String { uppercase ? text.uppercased() : text }
    private var selectedText: String? { spec.text(selection.direction).map(cased) }

    override func beginTracking(_ touch: UITouch, with event: UIEvent?) -> Bool {
        origin = touch.location(in: self)
        selection = SwipeSelection()
        trackingTouch = true
        onPreview?(selectedText)
        onSelection?()
        setNeedsDisplay()
        return true
    }
    override func continueTracking(_ touch: UITouch, with event: UIEvent?) -> Bool {
        update(touch)
        return true
    }
    private func update(_ touch: UITouch) {
        let location = touch.location(in: self)
        let previous = selection.direction
        selection.update(dx: location.x - origin.x, dy: location.y - origin.y)
        if previous != selection.direction {
            onPreview?(selectedText)
            onSelection?()
            setNeedsDisplay()
        }
    }
    override func endTracking(_ touch: UITouch?, with event: UIEvent?) {
        if let touch { update(touch) }
        let text = selectedText
        trackingTouch = false
        setNeedsDisplay()
        onPreview?(nil)
        // The gesture belongs to its starting key even across neighboring keys.
        // An unassigned direction produces nothing, rather than a surprise digit.
        if let text { onCommit?(text) }
    }
    override func cancelTracking(with event: UIEvent?) {
        trackingTouch = false
        onPreview?(nil)
        setNeedsDisplay()
    }
    override func accessibilityActivate() -> Bool {
        onCommit?(spec.digit)
        return true
    }

    override func draw(_ rect: CGRect) {
        let active = UIColor(red: 0.64, green: 0.88, blue: 0.86, alpha: 1)
        if trackingTouch {
            active.setFill()
            UIBezierPath(roundedRect: bounds.insetBy(dx: 2, dy: 3), cornerRadius: 17).fill()
        }
        let centerColor: UIColor = trackingTouch ? .init(white: 0.15, alpha: 1) : .white
        let letterColor: UIColor = trackingTouch ? .init(white: 0.25, alpha: 1) : .init(white: 0.75, alpha: 1)
        func label(_ text: String, center: CGPoint, size: CGFloat, color: UIColor, bold: Bool = false) {
            let attributes: [NSAttributedString.Key: Any] = [
                .font: UIFont.systemFont(ofSize: size, weight: bold ? .semibold : .regular),
                .foregroundColor: color
            ]
            let string = cased(text) as NSString
            let dimensions = string.size(withAttributes: attributes)
            string.draw(at: CGPoint(x: center.x - dimensions.width / 2, y: center.y - dimensions.height / 2), withAttributes: attributes)
        }
        let x = bounds.midX, y = bounds.midY
        label(spec.digit, center: .init(x: x, y: y), size: 23, color: centerColor)
        let positions: [(Direction, CGPoint)] = [
            (.left, .init(x: x - bounds.width * 0.36, y: y)),
            (.up, .init(x: x, y: y - 21)),
            (.right, .init(x: x + bounds.width * 0.36, y: y)),
            (.down, .init(x: x, y: y + 21))
        ]
        for (direction, position) in positions {
            guard let text = spec.text(direction) else { continue }
            let selected = trackingTouch && selection.direction == direction
            label(text, center: position, size: selected ? 17 : 13,
                  color: selected ? centerColor : letterColor, bold: selected)
        }
    }
}

private final class SymbolElement: UIAccessibilityElement {
    var activate: (() -> Void)?
    override func accessibilityActivate() -> Bool {
        activate?()
        return true
    }
}

private final class SymbolGrid: UIView {
    var onPreview: ((String?) -> Void)?
    var onCommit: ((String) -> Void)?
    var onSelection: (() -> Void)?
    private var selected: Int? { didSet { setNeedsDisplay() } }
    private let symbols = KeyMap.symbolRows.flatMap { $0 }

    override init(frame: CGRect) {
        super.init(frame: frame)
        contentMode = .redraw
        backgroundColor = UIColor(red: 0.23, green: 0.25, blue: 0.28, alpha: 1)
        accessibilityIdentifier = "symbol-grid"
        isMultipleTouchEnabled = false
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }
    private func cell(_ index: Int) -> CGRect {
        let width = bounds.width / 13, height = bounds.height / 4
        return CGRect(x: CGFloat(index % 13) * width, y: CGFloat(index / 13) * height, width: width, height: height)
    }
    func select(at point: CGPoint) {
        let index: Int?
        if bounds.contains(point) && bounds.width > 0 && bounds.height > 0 {
            index = min(3, Int(point.y / (bounds.height / 4))) * 13 + min(12, Int(point.x / (bounds.width / 13)))
        } else { index = nil }
        guard index != selected else { return }
        selected = index
        onPreview?(index.map { symbols[$0] })
        if index != nil { onSelection?() }
    }
    func finish(commit: Bool) {
        let text = selected.map { symbols[$0] }
        selected = nil
        onPreview?(nil)
        if commit, let text { onCommit?(text) }
    }
    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
        if let touch = touches.first { select(at: touch.location(in: self)) }
    }
    override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
        if let touch = touches.first { select(at: touch.location(in: self)) }
    }
    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
        if let touch = touches.first { select(at: touch.location(in: self)) }
        finish(commit: true)
    }
    override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) { finish(commit: false) }
    override func layoutSubviews() {
        super.layoutSubviews()
        accessibilityElements = symbols.enumerated().map { index, symbol in
            let element = SymbolElement(accessibilityContainer: self)
            element.accessibilityLabel = symbol == "⌫" ? "Delete" : symbol
            element.accessibilityIdentifier = "symbol-\(index)"
            element.accessibilityTraits = .keyboardKey
            element.accessibilityFrameInContainerSpace = cell(index)
            element.activate = { [weak self] in self?.onCommit?(symbol); self?.onSelection?() }
            return element
        }
    }
    override func draw(_ rect: CGRect) {
        for (index, symbol) in symbols.enumerated() {
            let frame = cell(index)
            let active = selected == index
            if active {
                UIColor(red: 0.64, green: 0.88, blue: 0.86, alpha: 1).setFill()
                UIBezierPath(roundedRect: frame.insetBy(dx: 1, dy: 3), cornerRadius: 10).fill()
            }
            let attributes: [NSAttributedString.Key: Any] = [
                .font: UIFont.systemFont(ofSize: min(22, frame.width * 0.66)),
                .foregroundColor: active ? UIColor(white: 0.15, alpha: 1) : UIColor.white
            ]
            let size = (symbol as NSString).size(withAttributes: attributes)
            (symbol as NSString).draw(at: CGPoint(x: frame.midX - size.width / 2, y: frame.midY - size.height / 2), withAttributes: attributes)
        }
    }
}

final class KeyboardView: UIView {
    static let contentHeight: CGFloat = 300
    var insertText: ((String) -> Void)?
    var deleteBackward: (() -> Void)?
    var moveCursor: ((Int) -> Void)?
    var nextKeyboard: (() -> Void)?
    var inputModeList: ((UIView, UIEvent) -> Void)?
    var feedbackEnabled = true
    private let preview = UILabel()
    private let symbolGrid = SymbolGrid(frame: .zero)
    private var symbolsVisible = false
    private var transientSymbols = false
    private var keys: [SwipeKey] = []
    private var controls: [String: KeyboardButton] = [:]
    private var systemPickerInstalled = false
    private var shift = 0 // off, one character, caps lock
    private var lastShiftTap: TimeInterval = 0
    private var deleteTimer: Timer?
    private var deleteRepeating = false
    private lazy var feedback = UISelectionFeedbackGenerator(view: self)
    private var idleText: String { symbolsVisible ? "Slide to a symbol, then lift" : "Swipe to a letter · tap for a number" }

    override init(frame: CGRect) {
        super.init(frame: frame)
        backgroundColor = UIColor(red: 0.23, green: 0.25, blue: 0.28, alpha: 1)
        isMultipleTouchEnabled = false
        preview.textColor = .init(white: 0.85, alpha: 1)
        preview.textAlignment = .center
        preview.font = .systemFont(ofSize: 13)
        preview.text = idleText
        preview.isAccessibilityElement = false
        addSubview(preview)
        for spec in KeyMap.keys {
            let key = SwipeKey(spec)
            key.accessibilityIdentifier = "key-\(spec.digit)"
            key.onCommit = { [weak self] text in self?.commit(text) }
            key.onPreview = { [weak self] text in
                guard let self else { return }
                self.preview.text = text ?? self.idleText
                self.preview.font = .systemFont(ofSize: text == nil ? 13 : 23, weight: .medium)
            }
            key.onSelection = { [weak self] in self?.tick() }
            keys.append(key)
            addSubview(key)
        }
        addControl("left", symbol: "arrowtriangle.left.fill", label: "Move cursor left") { [weak self] in self?.moveCursor?(-1) }
        addControl("right", symbol: "arrowtriangle.right.fill", label: "Move cursor right") { [weak self] in self?.moveCursor?(1) }
        addControl("shift", symbol: "shift", label: "Shift; double tap for caps lock") { [weak self] in self?.toggleShift() }
        addControl("minus", text: "−", label: "Hyphen") { [weak self] in self?.commit("-") }
        addControl("delete", symbol: "delete.left.fill", label: "Delete; hold to repeat") {}
        addControl("globe", symbol: "globe", label: "Next keyboard") { [weak self] in
            guard let self, !self.systemPickerInstalled else { return }
            self.nextKeyboard?()
        }
        addControl("comma", text: ",", label: "Comma") { [weak self] in self?.commit(",") }
        // Commit on touch-down so iOS's long-press handling cannot swallow a
        // held space. There is no language or layout gesture on this key.
        addControl("space", text: "space", label: "Space", event: .touchDown) { [weak self] in self?.commit(" ") }
        addControl("period", text: ".", label: "Period") { [weak self] in self?.commit(".") }
        addControl("return", symbol: "return", label: "Return") { [weak self] in self?.commit("\n") }
        addControl("symbols", text: "#+=", label: "Symbols") { [weak self] in
            guard let self else { return }
            self.showSymbols(!self.symbolsVisible)
        }
        controls["symbols"]?.titleLabel?.font = .systemFont(ofSize: 15, weight: .semibold)
        let symbolPan = UIPanGestureRecognizer(target: self, action: #selector(pickSymbol(_:)))
        symbolPan.maximumNumberOfTouches = 1
        controls["period"]!.addGestureRecognizer(symbolPan)
        symbolGrid.isHidden = true
        symbolGrid.onPreview = { [weak self] text in self?.preview.text = text ?? self?.idleText }
        symbolGrid.onSelection = { [weak self] in self?.tick() }
        symbolGrid.onCommit = { [weak self] text in
            guard let self else { return }
            if text == "⌫" { self.deleteBackward?() }
            else { self.commit(text) }
        }
        addSubview(symbolGrid)
        let delete = controls["delete"]!
        delete.addTarget(self, action: #selector(startDelete), for: .touchDown)
        delete.addTarget(self, action: #selector(finishDelete), for: .touchUpInside)
        delete.addTarget(self, action: #selector(cancelDelete), for: [.touchUpOutside, .touchCancel, .touchDragExit])
        delete.activateAccessibly = { [weak self] in self?.deleteBackward?(); self?.tick() }
        // System input-mode picker on long press, as on Apple's globe key.
        controls["globe"]!.addTarget(self, action: #selector(showInputModes(_:event:)), for: .allTouchEvents)
        controls["globe"]!.activateAccessibly = { [weak self] in self?.nextKeyboard?() }
        controls["space"]!.backgroundColor = .init(red: 0.31, green: 0.36, blue: 0.40, alpha: 1)
        controls["space"]!.layer.cornerRadius = 17
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

    private func addControl(_ id: String, text: String? = nil, symbol: String? = nil, label: String, event: UIControl.Event = .touchUpInside, action: @escaping () -> Void) {
        let button = KeyboardButton(type: .custom)
        button.accessibilityIdentifier = "key-\(id)"
        button.accessibilityLabel = label
        button.tintColor = .init(red: 0.72, green: 0.83, blue: 0.88, alpha: 1)
        button.setTitleColor(button.tintColor, for: .normal)
        button.titleLabel?.font = .systemFont(ofSize: id == "space" ? 15 : 25)
        if let symbol { button.setImage(UIImage(systemName: symbol), for: .normal) }
        if let text { button.setTitle(text, for: .normal) }
        button.addAction(UIAction { [weak self] _ in
            self?.tick()
            action()
        }, for: event)
        button.activateAccessibly = { [weak self] in self?.tick(); action() }
        controls[id] = button
        addSubview(button)
    }
    private func tick() {
        if feedbackEnabled { feedback.selectionChanged(); feedback.prepare() }
    }
    private func commit(_ text: String) {
        insertText?(text)
        if shift == 1 && text.unicodeScalars.contains(where: CharacterSet.letters.contains) {
            shift = 0
            updateShift()
        }
    }
    private func toggleShift() {
        let now = ProcessInfo.processInfo.systemUptime
        if shift == 1 && now - lastShiftTap < 0.35 { shift = 2 }
        else { shift = shift == 0 ? 1 : 0 }
        lastShiftTap = now
        updateShift()
    }
    private func updateShift() {
        keys.forEach { $0.uppercase = shift != 0 }
        controls["shift"]?.setImage(UIImage(systemName: shift == 2 ? "capslock.fill" : shift == 1 ? "shift.fill" : "shift"), for: .normal)
        controls["shift"]?.accessibilityValue = shift == 2 ? "Caps lock" : shift == 1 ? "On" : "Off"
    }
    @objc private func startDelete() {
        stopRepeating()
        deleteRepeating = false
        deleteTimer = Timer.scheduledTimer(withTimeInterval: 0.4, repeats: false) { [weak self] _ in
            guard let self else { return }
            self.deleteRepeating = true
            self.deleteBackward?()
            self.tick()
            self.deleteTimer = Timer.scheduledTimer(withTimeInterval: 0.08, repeats: true) { [weak self] _ in
                self?.deleteBackward?()
            }
        }
    }
    @objc private func finishDelete() {
        if !deleteRepeating { deleteBackward?() }
        stopRepeating()
    }
    @objc private func cancelDelete() { stopRepeating() }
    func stopRepeating() { deleteTimer?.invalidate(); deleteTimer = nil }
    override func didMoveToWindow() {
        super.didMoveToWindow()
        if window == nil {
            stopRepeating()
            symbolGrid.finish(commit: false)
            if transientSymbols { showSymbols(false); transientSymbols = false }
        } else if feedbackEnabled { feedback.prepare() }
    }
    private func showSymbols(_ show: Bool) {
        symbolGrid.finish(commit: false)
        symbolsVisible = show
        symbolGrid.isHidden = !show
        keys.forEach { $0.isHidden = show }
        for id in ["minus", "left", "right", "shift", "delete"] { controls[id]?.isHidden = show }
        controls["symbols"]?.setTitle(show ? "2ABC" : "#+=", for: .normal)
        controls["symbols"]?.accessibilityLabel = show ? "Letters and numbers" : "Symbols"
        preview.text = idleText
        preview.font = .systemFont(ofSize: 13)
        setNeedsLayout()
        layoutIfNeeded()
    }
    @objc private func pickSymbol(_ pan: UIPanGestureRecognizer) {
        switch pan.state {
        case .began:
            transientSymbols = !symbolsVisible
            showSymbols(true)
            symbolGrid.select(at: pan.location(in: symbolGrid))
        case .changed:
            symbolGrid.select(at: pan.location(in: symbolGrid))
        case .ended, .cancelled, .failed:
            if pan.state == .ended { symbolGrid.select(at: pan.location(in: symbolGrid)) }
            symbolGrid.finish(commit: pan.state == .ended)
            if transientSymbols { showSymbols(false) }
            transientSymbols = false
        default: break
        }
    }
    @objc private func showInputModes(_ sender: UIView, event: UIEvent) {
        inputModeList?(sender, event)
    }
    func configureGlobe(systemPicker: Bool) {
        // With a system picker installed, it owns both taps and holds.
        controls["globe"]?.isHidden = false
        systemPickerInstalled = systemPicker
    }
    override func layoutSubviews() {
        super.layoutSubviews()
        guard bounds.width > 0, bounds.height > 88 else { return }
        let width = bounds.width, column = width / 5
        controls["symbols"]?.frame = CGRect(x: 4, y: 0, width: 60, height: 34)
        preview.frame = CGRect(x: 68, y: 0, width: width - 72, height: 34)
        preview.adjustsFontSizeToFitWidth = true
        preview.minimumScaleFactor = 0.7
        symbolGrid.frame = CGRect(x: 0, y: 34, width: width, height: bounds.height - 34 - 54)
        let rowHeight = (bounds.height - 34 - 54) / 3
        func cell(_ columnIndex: Int, _ row: Int) -> CGRect {
            CGRect(x: CGFloat(columnIndex) * column, y: 34 + CGFloat(row) * rowHeight, width: column, height: rowHeight)
        }
        keys[0].frame = cell(0, 0)
        for number in 1...9 { keys[number].frame = cell(1 + (number - 1) % 3, (number - 1) / 3) }
        for (id, col, row) in [("minus", 4, 0), ("left", 0, 1), ("right", 4, 1), ("shift", 0, 2), ("delete", 4, 2)] {
            controls[id]?.frame = cell(col, row)
        }
        let bottom = bounds.height - 54
        controls["globe"]?.frame = CGRect(x: 0, y: bottom, width: column, height: 54)
        controls["comma"]?.frame = CGRect(x: column, y: bottom, width: column * 0.65, height: 54)
        controls["space"]?.frame = CGRect(x: column * 1.65, y: bottom + 3, width: column * 1.7, height: 48)
        controls["period"]?.frame = CGRect(x: column * 3.35, y: bottom, width: column * 0.65, height: 54)
        controls["return"]?.frame = CGRect(x: column * 4, y: bottom, width: column, height: 54)
    }
}
