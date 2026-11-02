import UIKit

final class KeyboardViewController: UIInputViewController {
    private let keyboard = KeyboardView(frame: .zero)

    override func viewDidLoad() {
        super.viewDidLoad()
        keyboard.feedbackEnabled = hasFullAccess
        keyboard.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(keyboard)
        let height = view.heightAnchor.constraint(equalToConstant: KeyboardView.contentHeight)
        height.priority = .init(999)
        NSLayoutConstraint.activate([
            keyboard.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            keyboard.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            keyboard.topAnchor.constraint(equalTo: view.topAnchor),
            keyboard.bottomAnchor.constraint(equalTo: view.bottomAnchor), height
        ])
        keyboard.insertText = { [weak self] in self?.textDocumentProxy.insertText($0) }
        keyboard.deleteBackward = { [weak self] in self?.textDocumentProxy.deleteBackward() }
        keyboard.moveCursor = { [weak self] in self?.textDocumentProxy.adjustTextPosition(byCharacterOffset: $0) }
        keyboard.nextKeyboard = { [weak self] in self?.advanceToNextInputMode() }
        keyboard.inputModeList = { [weak self] sender, event in
            self?.handleInputModeList(from: sender, with: event)
        }
        keyboard.configureGlobe(systemPicker: true)
    }
    override func viewWillDisappear(_ animated: Bool) {
        keyboard.stopRepeating()
        super.viewWillDisappear(animated)
    }
    override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        keyboard.feedbackEnabled = hasFullAccess
    }
}
