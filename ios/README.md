# Speak Pocket

Offline type-to-speak for iPhone, with a Rust/Dioxus UI and a Rust speech worker
calling sherpa-onnx. Emma speaks British English; Xiaoxiao speaks Mandarin.
The existing Python/MLX CLI remains available at the workspace root.

The UI includes Speak, Stop, Repeat, speed control, and four phrase buttons per
language. Select the voice for the text you are entering. Nothing is sent to a
server. Dioxus renders the Rust UI in the platform WebView; inference and audio
playback are native. A small Objective-C function configures the iOS playback
audio session so speech works with the silent switch enabled.

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
ignored by Git. The resulting offline app is roughly 189 MB.

```sh
cargo test --manifest-path ios/Cargo.toml --release --lib
cargo clippy --manifest-path ios/Cargo.toml --release --all-targets -- -D warnings
cargo fmt --manifest-path ios/Cargo.toml --check
```

The native engine runs on one dedicated worker thread and remains loaded between
requests. Stop immediately invalidates pending results and stops playback; an
in-progress inference can only cancel at the engine's callback boundaries.
New speech replaces the previous request. Generation finishes before playback
in this first prototype. The text box remains editable while speech is running.

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

## Validation and current limits

Verified on the development Mac: real English and Chinese generation/playback,
finite non-silent 24 kHz output, Rust input-validation tests, Clippy, desktop UI
startup, arm64 iOS compilation, and unsigned Xcode packaging. The initial speech
check generated 3.27 seconds of English in 3.45 seconds and 4.31 seconds of
Mandarin in 4.65 seconds; these are Mac observations, not iPhone benchmarks.

**Physical iPhone playback is not yet verified.** At initial setup, the phone
was unavailable and this Mac had zero signing identities. No simulator runtime
was installed either. Phone-specific checks still needed: Chinese keyboard
composition, audio interruptions/headphone routing, background/foreground
transitions, and cold/warm synthesis latency.

This is a foreground prototype: no background-audio entitlement, custom saved
phrases, persistent settings, phrase-audio cache, or App Store packaging yet.
The sherpa lexicon frontend differs from Python Misaki; compare pronunciation
of names, uncommon words, and mixed-language text before relying on it daily.

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
