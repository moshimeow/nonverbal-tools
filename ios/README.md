# Speak Pocket

Offline type-to-speak for iPhone, with a Rust/Dioxus UI and a Rust speech worker
calling sherpa-onnx. Emma speaks British English; Xiaoxiao speaks Mandarin.
The existing Python/MLX CLI remains available at the workspace root.

Play sends the draft to the bottom of a FIFO queue and clears the composer.
Swipe any queue row left to stop/remove it; the × button does the same thing.
The two most recently completed messages stay above the queue. Tap one to add a
new copy at the bottom, keeping its original voice and speed. Open ☰ for language,
voice, and speed settings. Settings apply to newly sent messages.

Nothing is sent to a server. Dioxus renders the Rust UI in the platform WebView;
inference and audio playback are native. English uses a Misaki-style Rust
phonemizer with an embedded neural fallback for names, typos, and invented words.
Mandarin uses the bilingual model's Chinese lexicon and number normalization.

## Development

Run these commands from the workspace root on an Apple Silicon Mac with Xcode,
Rust, and Python 3 (used only by the bundle-staging helper):

```sh
./ios.sh setup          # pinned local Dioxus CLI, Rust targets, bilingual model
./ios.sh check --play   # generate and play both voices using Rust
./ios.sh desktop       # Rust UI on the Mac, with Dioxus file watching
./ios.sh build         # unsigned arm64 iPhone app, iOS 18+
```

The local Dioxus executable is `.tools/dioxus/dx`. Use the wrapper because `dx`
elsewhere on this Mac belongs to Deno. Cargo.lock is committed. The setup script
verifies the CLI/model archive hashes; model downloads and build products are
ignored by Git. The resulting offline app is roughly 211 MB.

```sh
cargo test --manifest-path ios/Cargo.toml --release --lib
cargo clippy --manifest-path ios/Cargo.toml --release --all-targets -- -D warnings
cargo fmt --manifest-path ios/Cargo.toml --check
# Real audio, including overlap and cancellation (speaks on the Mac):
cargo run --manifest-path ios/Cargo.toml --release --bin queue-check
# With the desktop app running and Accessibility permission granted:
osascript ios/scripts/ui-smoke.applescript
```

A dedicated synthesis thread owns the model and phonemizer. A separate playback
thread handles the queue and audio output, preparing at most one item ahead of
the current message. Removing a playing item stops its sink promptly; removing a
generating item invalidates its result and requests cancellation at the next
inference callback. Failed rows remain removable and do not block later messages.
History contains completed messages only. The queue and history are in memory.

On iPhone, the native session uses `playAndRecord`, `defaultToSpeaker`, and an
explicit speaker override before playback. Route-change notifications reapply
that override when accessories connect. No input stream is opened or microphone
audio captured; Apple's speaker override requires this session category. Bluetooth
and AirPlay routing options are not enabled. System interruptions can still stop
speech; this is not a background-audio or phone-call override.

## Install on your iPhone

The unsigned bundle is `ios/dist/SpeakPocket.app`. Apple requires a development
certificate and provisioning profile before it can run on a real phone.

1. In **Xcode → Settings → Apple Accounts**, sign in with your Apple ID. Select
   your team and use **Manage Certificates** to create an Apple Development
   certificate if one is not already present.
2. Connect and unlock the iPhone, trust this Mac, and enable Developer Mode on
   the phone if prompted. Allow Xcode to finish preparing the device. If Xcode
   requests iOS platform components, install those through Settings → Components.
3. Run `./ios.sh build`, then open `ios/SpeakPocket.xcodeproj`. Select the
   SpeakPocket target, **Signing & Capabilities**, and your team with automatic
   signing enabled. This small Xcode project packages the already-built Rust app;
   application code is still built by Cargo/Dioxus.
4. Select the phone in Xcode and Run for the first installation. If Xcode changes
   the bundle identifier because it is unavailable, update the matching identifier
   in `Dioxus.toml`, `scripts/package.py`, and `ios.sh` as well.

After the account/device is provisioned, use the CLI:

```sh
xcrun devicectl list devices
APPLE_TEAM_ID=YOUR_TEAM_ID IOS_DEVICE_ID=YOUR_DEVICE_ID ./ios.sh deploy
```

The deploy command builds with Dioxus, stages the model and the iPhone slice of
SherpaOnnxC.framework, lets Xcode sign both, then installs and launches with
`devicectl`. Device pairing/account login must be completed interactively.
The CLI resolves the CoreDevice identifier to the hardware UDID Xcode requires
and allows Xcode to register that destination with your signing team.
It cleans the small signing wrapper before staging, so Xcode always signs the
newly copied Rust executable. Cargo's compiled artifacts remain cached.
For a personal development team, iOS may require trusting the developer under
**Settings → General → VPN & Device Management** before the first launch.

## Validation and current limits

Verified on the development Mac: English, invented-word and Mandarin synthesis
and playback; FIFO queue order; generation overlapping playback; responsive
current-item cancellation; removal without replaying stale audio; two-item
history; Rust tests and Clippy. The updated UI was inspected at phone width and
send/clear/queue insertion were exercised. The full UI smoke script requires an
empty composer and refuses to overwrite an existing draft.

The previous iPhone build's playback was confirmed by the user. The queue update
was built, signed, installed and launched on their iPhone 15 Pro Max. The speaker
override compiles for iPhone; connected-AirPods routing and mid-sentence accessory
changes still need a physical test. Chinese keyboard composition, interruptions,
background/foreground transitions and phone synthesis latency need more testing.

This remains a foreground prototype: no saved phrases, persistent settings,
phrase-audio cache, or App Store packaging. Unfamiliar-word pronunciations are
best effort. English and Mandarin have separate frontends; mixed-language text
is not automatically language-detected.

## Dependencies and model provenance

- [Dioxus 0.7.10](https://github.com/DioxusLabs/dioxus): Rust UI and CLI.
- [sherpa-onnx 2.0.0](https://docs.rs/sherpa-onnx/2.0.0/sherpa_onnx/): Rust API
  over the native speech runtime; iOS uses its prebuilt shared framework.
- [Bilingual Kokoro v1.0](https://k2-fsa.github.io/sherpa/onnx/tts/pretrained_models/kokoro.html):
  int8 ONNX model, British/Chinese lexicons, Chinese number rules and dictionary.
  Speaker IDs 21 and 47 correspond to `bf_emma` and `zf_xiaoxiao` in the
  [v1.0 voice table](https://github.com/k2-fsa/sherpa-onnx/blob/master/scripts/kokoro/v1.0/generate_voices_bin.py).
- Rodio/CPAL handle native playback. The model's license is copied into the app
  alongside its weights. Model/framework versions and voice mappings are coupled;
  do not swap a v1.1 voice pack into this v1.0 configuration.

- [loqui-g2p 0.4.0](https://github.com/Awakened-Labs/loqui): British Misaki-style
  English phonemization with embedded neural out-of-vocabulary prediction. Its
  license and notices are bundled in `licenses/loqui-g2p/` inside the app.
- [Apple speaker override](https://developer.apple.com/documentation/avfaudio/avaudiosession/overrideoutputaudioport(_:)):
  native speaker routing, reapplied after route changes.
