import UIKit
import AVFoundation

@main
final class AppDelegate: UIResponder, UIApplicationDelegate {
    var window: UIWindow?
    func application(_ application: UIApplication, didFinishLaunchingWithOptions options: [UIApplication.LaunchOptionsKey: Any]?) -> Bool {
        window = UIWindow(frame: UIScreen.main.bounds)
        window?.rootViewController = PracticeViewController()
        window?.makeKeyAndVisible()
        return true
    }
}

final class PracticeViewController: UIViewController {
    private let editor = UITextView()
    private let keyboard = KeyboardView(frame: CGRect(x: 0, y: 0, width: 390, height: KeyboardView.contentHeight))
    private let speech = AVSpeechSynthesizer()
    private var usingPracticeKeyboard = true

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .systemBackground
        let title = UILabel()
        title.text = "2ABC"
        title.font = .systemFont(ofSize: 34, weight: .bold)
        let instructions = UILabel()
        instructions.text = "Swipe toward a letter, then lift. Tap for a number.\n\nSymbols: tap #+=, or swipe from period to a symbol and release."
        instructions.numberOfLines = 0
        instructions.font = .systemFont(ofSize: 16)
        instructions.textColor = .secondaryLabel
        let install = UILabel()
        install.text = "Add 2ABC in Settings → General → Keyboard → Keyboards, then hold 🌐 to choose it.\n\nFor swipe haptics in other apps, enable Allow Full Access for 2ABC. Typing works without it. This keyboard sends no data over the network."
        install.numberOfLines = 0
        install.font = .systemFont(ofSize: 14)
        install.textColor = .secondaryLabel
        editor.font = .systemFont(ofSize: 26)
        editor.backgroundColor = .secondarySystemBackground
        editor.layer.cornerRadius = 16
        editor.textContainerInset = UIEdgeInsets(top: 16, left: 12, bottom: 16, right: 12)
        editor.accessibilityLabel = "Practice text"
        editor.accessibilityIdentifier = "practice-editor"
        editor.autocorrectionType = .no
        editor.autocapitalizationType = .none
        editor.smartQuotesType = .no
        editor.smartDashesType = .no
        editor.inputView = keyboard
        keyboard.autoresizingMask = [.flexibleWidth]
        keyboard.insertText = { [weak self] in self?.editor.insertText($0) }
        keyboard.deleteBackward = { [weak self] in self?.editor.deleteBackward() }
        keyboard.moveCursor = { [weak self] offset in
            guard let editor = self?.editor, let range = editor.selectedTextRange,
                  let position = editor.position(from: range.start, offset: offset) else { return }
            editor.selectedTextRange = editor.textRange(from: position, to: position)
        }
        keyboard.nextKeyboard = { [weak self] in self?.toggleKeyboard() }
        let buttons = UIStackView()
        buttons.axis = .horizontal
        buttons.spacing = 12
        buttons.distribution = .fillEqually
        for (text, action) in [
            ("Speak", UIAction { [weak self] _ in self?.speak() }),
            ("Stop", UIAction { [weak self] _ in self?.speech.stopSpeaking(at: .immediate) }),
            ("Keyboard", UIAction { [weak self] _ in self?.toggleKeyboard() })
        ] {
            var config = UIButton.Configuration.tinted()
            config.title = text
            let button = UIButton(configuration: config, primaryAction: action)
            button.accessibilityIdentifier = "practice-\(text.lowercased())"
            buttons.addArrangedSubview(button)
        }
        let stack = UIStackView(arrangedSubviews: [title, instructions, install, editor, buttons])
        stack.axis = .vertical
        stack.spacing = 14
        stack.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(stack)
        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.leadingAnchor, constant: 20),
            stack.trailingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.trailingAnchor, constant: -20),
            stack.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor, constant: 12),
            stack.bottomAnchor.constraint(equalTo: view.keyboardLayoutGuide.topAnchor, constant: -12),
            editor.heightAnchor.constraint(greaterThanOrEqualToConstant: 90),
            buttons.heightAnchor.constraint(equalToConstant: 44)
        ])
        title.setContentHuggingPriority(.required, for: .vertical)
        instructions.setContentHuggingPriority(.required, for: .vertical)
        install.setContentHuggingPriority(.required, for: .vertical)
    }
    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        editor.becomeFirstResponder()
    }
    private func toggleKeyboard() {
        usingPracticeKeyboard.toggle()
        editor.inputView = usingPracticeKeyboard ? keyboard : nil
        editor.reloadInputViews()
        editor.becomeFirstResponder()
    }
    private func speak() {
        guard !editor.text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return }
        speech.stopSpeaking(at: .immediate)
        let utterance = AVSpeechUtterance(string: editor.text)
        utterance.voice = AVSpeechSynthesisVoice(language: "en-GB")
        speech.speak(utterance)
    }
}
