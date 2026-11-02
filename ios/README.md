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
English words containing only uppercase ASCII letters (at least two) are spelled
out using British letter names: `NASA`, `CPU`, `API`, and even `HELLO`. Single `I`
and `A` stay normal words. A question mark, including `？`, ends an English synthesis
segment so the following statement cannot determine its ending contour. Kokoro
still controls prosody; this is not a guaranteed or artificially applied pitch rise.

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
ignored by Git. The resulting offline app is roughly 412 MB.

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
The two replay cards contain completed messages only and stay in memory. The
History tab separately saves **every sent message**, including replayed, canceled,
and failed messages, in a local SQLite database in Application Support. It survives
relaunches and app updates; uninstalling the app removes it. History starts with
this version because earlier builds did not persist a transcript. Original text is
preserved. Entries are selectable, have a native Copy button, and Copy all exports
the complete transcript in chronological order. The list shows newest first and
loads 100 rows at a time. Switching tabs keeps the unsent draft intact.

On iPhone, the native session uses `playAndRecord`, `defaultToSpeaker`, and an
speaker override when needed before playback. If the output changes away from
the speaker during speech, playback stops with a visible error. No input stream is opened or microphone
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

The int8 model produced all-NaN audio for some short inputs (`woof!`, `Hello!`,
`floof`). The full-precision model fixes those cases without altering the words.
On-device diagnostics verified valid 24 kHz output for `woof!`, `woof`, `Hello!`,
`Hello world.`, `zorbulate`, `floof`, and longer sentences. `./ios.sh check` includes
short-word regressions and fails if any sample is invalid. To repeat synthesis
checks on a connected iPhone, launch with the diagnostic argument (then relaunch
normally afterward):

```sh
xcrun devicectl device process launch --device YOUR_DEVICE_ID --terminate-existing \
  --console com.moshi.speakpocket --speech-check
```

The previous iPhone build's playback was confirmed by the user. The queue update
was built, signed, installed and launched on their iPhone 15 Pro Max. The speaker
override is built and installed; connected-AirPods routing and mid-sentence accessory
changes still need a physical test. Chinese keyboard composition, interruptions,
background/foreground transitions and phone synthesis latency need more testing.

This remains a foreground prototype: no saved phrase presets, persistent settings,
phrase-audio cache, or App Store packaging. Unfamiliar-word pronunciations are
best effort. English and Mandarin have separate frontends; mixed-language text
is not automatically language-detected.

## Dependencies and model provenance

- [Dioxus 0.7.10](https://github.com/DioxusLabs/dioxus): Rust UI and CLI.
- [sherpa-onnx 2.0.0](https://docs.rs/sherpa-onnx/2.0.0/sherpa_onnx/): Rust API
  over the native speech runtime; iOS uses its prebuilt shared framework.
- [Bilingual Kokoro v1.0](https://k2-fsa.github.io/sherpa/onnx/tts/pretrained_models/kokoro.html):
  full-precision ONNX model, British/Chinese lexicons, Chinese number rules and dictionary.
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

- SQLite (via bundled rusqlite) stores the local sent-message transcript; UIKit/AppKit handle clipboard writes.

### Write and message metadata

The pencil (Write) tab is a silent, newest-first conversation sheet. The speaker icon returns to Speak;
the clock (History) lives in the hamburger menu. The text-size buttons share the top row
with the outline-icon tabs. The live draft sits at
its top; Enter or Send saves it and opens a fresh draft above it. The − / + buttons
change text size. Drag a message's small horizontal handle down or up to change
only the blank space above that message. Spacing and messages survive a restart;
there is no message-count limit or pagination in Write. A focused handle also
accepts the up/down arrow keys for spacing.

Enter (including Shift-Enter) submits in Speak and Write. Enter used to confirm
Chinese IME composition does not send the message. Write messages share the
copyable History tab with spoken messages, labeled by their source.

History shows local date and time for all messages, including existing history.
New sends request a one-shot Core Location fix when permission is allowed; the
history shows coordinates and estimated accuracy. This does not track location
in the background. Denial, unavailable fixes, and old messages are shown as
“Location not recorded”; the app never fills old messages with a later location.
Messages remain usable without location permission. Coordinates, text, timestamps,
and spacing stay in the existing local SQLite history.

Additional checks:

```sh
node ios/scripts/interaction-check.cjs
cargo test --locked --manifest-path ios/Cargo.toml --lib
# Launch desktop with an isolated SPEAK_DATA_DIR before running this UI test:
osascript ios/scripts/write-smoke.applescript
```

### Music pause and resume

Speech uses a non-mixing audio session: other audio pauses while Speak Pocket
speaks on the iPhone speaker. Once the queue empties, the session deactivates with
`notifyOthersOnDeactivation`, allowing the music app to resume on its own route.
Resumption ultimately depends on the other app. The session stays active between
queued messages, including while the next message finishes generating, to avoid
repeated pause/resume transitions.

Rust owns synthesis and the queue. A dedicated native audio thread owns the
AVAudioEngine and audio session. Starting playback returns a ticket immediately;
“Preparing audio…” changes to “Speaking…” once native playback starts. Removing a
message during setup flags it for cancellation without blocking typing or queue
operations. Completion callbacks and a duration-plus-three-seconds watchdog
report playback success or failure. Desktop playback continues to use Rodio.

Session release failures retain the active state, appear in the UI, and get up to
three delayed retries. Interrupted or failed speech is not added to the completed
replay cards. A later message can attempt activation again.

The earlier mixing and dualRoute experiments are preserved on
`backup/dualroute-experiment`. The dualRoute version played audibly in user testing
but paused and resumed music anyway. This implementation removes its channel
mapping, dynamic symbol lookup, diagnostic launch flags, and iOS 26.2 requirement.

On-device acceptance check for this simplified implementation:

1. Play music through AirPods and send “woof!”. Confirm speech uses the phone
   speaker, music pauses, and music resumes through AirPods afterward.
2. Queue several messages. Confirm music stays paused between them and resumes
   after the last one.
3. Remove a message during preparation and during playback. Check that typing
   stays responsive and music resumes once no speech remains queued.
4. Repeat without headphones, then with AirPods disconnected/reconnected.

Builds and desktop tests do not establish iPhone routing behavior. After the
simplified implementation was installed, the user confirmed that it worked on
their iPhone and approved merging it on 2026-10-25.

### Renewing a personal development installation

Free Personal Team profiles expire after seven days. Re-sign and install over the
existing app with the same bundle identifier; do not uninstall it, since that
removes local history. If Finder trusts the phone but Xcode says it is offline,
check VPNs: Mullvad on the phone blocked the developer USB network connection in
our November 1 refresh. Keep the phone unlocked through installation. If the new
installation is rejected at launch, check Settings → General → VPN & Device
Management for the developer's Trust / Verify App action. The November 1 refresh
installed successfully and the user confirmed that Trust restored access.
