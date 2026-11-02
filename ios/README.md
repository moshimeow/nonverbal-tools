# Speak Pocket

Offline type-to-speak for iPhone, with a Rust/Dioxus UI and a Rust speech worker
calling sherpa-onnx. Emma speaks British English; Xiaoxiao speaks Mandarin.
The existing Python/MLX CLI remains available at the workspace root.

The conversation sheet has one growing composer above newest-first sent messages.
Hold anywhere on the conversation surface for 550 ms to switch Speak/Write. The
send icon, playback behavior, and text size change; the draft and transcript stay
put. The menu provides an accessible mode switch. On desktop, Tab switches modes
while keeping the composer focused; Shift–Tab navigates controls. Tab retains its
normal behavior inside settings/History and during IME composition.
The − / + controls adjust the current mode's size independently (defaults: Speak
20 px, Write 36 px); mode and both sizes persist locally. Language, voice, speed,
and copyable History live in ☰. The small left segment of Send submits once in the
opposite modality without changing mode. macOS disables press-and-hold accents
for this app only; letter repeats use the system's repeat delay/rate.

Speak sends enter a FIFO audio queue; Write sends remain silent. Each saved message
has a speaker button to play it using the current voice/speed, or a Stop button
while queued/playing. Replay uses the same visible message instead of adding a
second card. Each message stores its latest playback outcome, including failures
and partial playback; failures leave the active queue automatically and appear
beneath the message, with nothing to dismiss. Drag its handle to add spacing
above it. All sent messages and spacing survive restart.

Nothing is sent to a server. Dioxus renders the Rust UI in the platform WebView;
inference and audio playback are native. English uses a Misaki-style Rust
phonemizer with an embedded neural fallback for names, typos, and invented words.
Mandarin uses Jieba word segmentation and the bilingual model's Chinese lexicon,
with explicit phoneme input and common numeric-form normalization.
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
./ios.sh desktop-build # self-contained ios/dist/macos/Speak Pocket.app
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

A dedicated synthesis thread owns the model and phonemizer, loading them on app
launch. A separate coordinator handles playback and persistence. It streams one
phrase at a time, buffering at most two phrases each for the current/next message.
Stopping invalidates in-flight inference and discards late results. Native audio
setup remains asynchronous, so Stop and typing do not wait on audio activation.

History saves every submitted message, its source, and its latest playback result
in SQLite in Application Support. Replaying updates that result on the existing
message, rather than recording another submission. History preserves original
text, offers native Copy and Copy all, and loads 100 rows at a time. Opening
History keeps the unsent draft intact. Updating preserves data; uninstalling
removes it. In-flight speech from a previous launch is marked not played and is
never automatically replayed.

On iPhone, the native session uses `playAndRecord`, `defaultToSpeaker`, and a
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

The unified version passes Rust queue/cache/history tests, Clippy, JavaScript
interaction tests (Enter/IME, hold/cancel/click suppression, auto-growth, spacing),
and real English/Mandarin FIFO playback with cancellation and overlap. The native
macOS UI was inspected at phone width. Global keyboard UI automation is avoided
because the user actively changes window focus; the native smoke script is
read-only. Physical iPhone gesture/playback acceptance remains a user test.

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

This remains a foreground app, with local-only inference and no App Store
packaging. Mode/text sizes persist; language and speed reset on launch. There is
no automatic mixed-language detection. Native IME behavior, interruptions, and
background transitions still benefit from physical-device testing.

### Lower-latency speech

Kokoro's neural model evaluates a complete segment; its LSTM does not expose a
reusable streaming state for arbitrary text edits. The standard Python pipeline
packs roughly 510 phoneme characters with punctuation-aware breaks; our English
frontend keeps its existing 400-phoneme safety limit. See the upstream
[model](https://github.com/hexgrad/kokoro/blob/main/kokoro/model.py) and
[pipeline](https://github.com/hexgrad/kokoro/blob/main/kokoro/pipeline.py).

The app now splits text into shorter phrases before inference: sentence endings,
then sufficiently long comma/semicolon clauses, then word boundaries at roughly
160 English or 60 Mandarin characters. Extremely long unbroken tokens are split
at a Unicode boundary. The first phrase can play while later phrases generate.
This is phrase streaming, not frame-by-frame streaming within the model. Shorter
context can change prosody, and playback may wait at a boundary if generation
falls behind. The composer and sends no longer have the old 2,000-character cap.

After a 650 ms typing pause in Speak mode, idle inference prepares the first draft
phrase without playing or recording it. A four-entry, eight-megabyte RAM cache
requires an exact phrase/voice/speed match. Submitted work takes priority over
speculation; changed drafts invalidate in-flight preparation, and stale results
cannot reach playback. Write mode does not prepare speech. The cache is not saved.

On this development Mac, a sample English message took 3.680 s to generate whole
versus 0.313 s for its first phrase (four phrases total: 4.135 s for 10.9 s audio).
Mandarin took 4.947 s whole versus 0.550 s first phrase (4.926 s for 11.0 s audio).
These are warm-model generation measurements, not iPhone tap-to-sound numbers.
Repeat with:

```sh
cargo run --locked --release --manifest-path ios/Cargo.toml --bin latency-check
```

The desktop package includes the model and licenses in Contents/Resources and
uses the exact same interface. It is locally ad-hoc signed, not notarized for
public distribution; it can be opened directly or moved into Applications.

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
  native speaker routing with route-change failure handling.

- SQLite (via bundled rusqlite) stores the local sent-message transcript; UIKit/AppKit handle clipboard writes.

### Write and message metadata

The unified sheet displays all messages, newest first. Spacing handles move only
the blank space above a message, never reorder it. They also accept up/down arrow
keys. There is no message-count limit or pagination on the conversation sheet.
Enter (including Shift-Enter) submits in either mode; Enter used to confirm IME
composition does not send. Long-press excludes spacing drags and the History/
settings panels so those retain their normal interactions.

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
# Read-only native control inspection; does not change window focus:
osascript ios/scripts/ui-smoke.applescript
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
three delayed retries. Interrupted or failed speech is recorded beneath its message. A later message can attempt activation again.

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

### Mandarin frontend correction

The sherpa-onnx 2.0.0 lexicon path constructs an end-PAD-terminated token sequence,
then appends trailing punctuation after that PAD. It also concatenates lexicon
pronunciations without the word spaces used by Kokoro's Python frontend. The
Mandarin frontend now supplies explicit IPA via `phoneme_codepoints`, as English
already does, preserving word boundaries and letting sherpa frame the sequence
once. The exact reported question produced 1.665 s through the old path versus
1.460 s with Python-reference phonemes. This confirms different model input and
audio; subjective tone quality still requires listening.

The pinned MIT-licensed `jieba-rs` 0.11.0 uses the already-bundled dictionary.
The frontend preserves lexical tones and common numeric forms (cardinals, years,
decimals, percentages, clock times, digit strings). It is not a comprehensive
Chinese text normalizer or tone-sandhi implementation. Unknown Han characters
report a pronunciation error instead of silently disappearing. Original written
text, including 调 rather than 跳 in the regression sample, stays unchanged.
Kokoro v1.0 itself has limited Mandarin training, so fixing token framing does
not establish native-quality tones. See the upstream
[voice-quality notes](https://huggingface.co/hexgrad/Kokoro-82M/blob/main/VOICES.md)
and [lexicon/token framing implementation](https://github.com/k2-fsa/sherpa-onnx/blob/v2.0.0/sherpa-onnx/csrc/offline-tts-kokoro-impl.h).

Reproduce old versus corrected whole/chunked speech without touching history:

```sh
cargo run --locked --release --manifest-path ios/Cargo.toml --bin mandarin-check -- \
  ios/tests/mandarin-cases.tsv /tmp/speak-mandarin-comparison
```

The emitted numbered WAV files correspond to fixture rows. Desktop key repeat
and iPhone speech quality are manual acceptance checks; automated tests do not
steal keyboard focus from the user.
