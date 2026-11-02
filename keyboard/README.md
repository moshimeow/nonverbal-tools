# 2ABC keyboard prototype

A native Swift/UIKit keyboard extension, selectable in other iPhone apps, plus
a small practice app. The layout follows the supplied video's front page before
02:26. It is independent of Speak Pocket and its Rust/Dioxus speech backend.

Start a touch anywhere on a numbered key, swipe toward the printed letter, then
lift to insert it. A short tap inserts the number. A swipe remains attached to
its starting key even when the finger crosses another key. Move back to the touch
origin before lifting to insert the number instead. An unused direction inserts
nothing; a system-cancelled gesture inserts nothing.

| Key | Left | Up | Right | Down |
| --- | --- | --- | --- | --- |
| 1 | ' | @ | " | * |
| 2 | a | b | c | æ |
| 3 | d | e | f | # |
| 4 | g | h | i | — |
| 5 | j | k | l | ÷ |
| 6 | m | n | o | œ |
| 7 | p | q | r | s |
| 8 | t | u | v | ~ |
| 9 | w | x | y | z |

0 is on the upper left, matching the reference. Cursor arrows, shift, delete,
hyphen, comma, period, space, and return surround the grid. Shift applies to one
letter; a quick double tap locks capitals. Hold delete to repeat. The globe opens
the iOS keyboard picker. Space always inserts a space, including after a hold.
The preview strip shows the current choice before release. Selection haptics use
a UIKit feedback generator attached to the keyboard view. For haptics in the
extension, enable **Allow Full Access** under the 2ABC keyboard's iOS Settings.
Typing and symbol selection work without that permission. Physical iPhone trials
confirmed haptics in the practice app and in the extension after Full Access
was enabled.

The directional labels and alphabet are visible in the video. Tap-to-digit and
the precise gesture thresholds are prototype interpretations: 12 points to
select a direction, with a 3-point hysteresis to reduce accidental switching.
There is no prediction/autocorrection, language selector, or clipboard gesture
in this version. The 2ABC layout stays fixed.

## Symbols

Tap **#+=** at the upper left to open the symbols page. Tap a symbol, or touch
and slide across the grid before releasing to insert the highlighted symbol.
Tap **2ABC** in the same place to return to the numbered keys. The page stays
open while entering multiple symbols; space still inserts only a space.

For one symbol, swipe from **period** into the symbol grid and lift. The chosen
symbol is inserted and the numbered keys return automatically. Release outside
the grid to cancel. A plain period tap still inserts a period.

The four rows follow the reference video, including punctuation, brackets,
currency signs, quotes, mathematical signs, and dashes. The video's lower-left
symbol-page shift is adapted to **Delete**. There is no language selector or
alternate symbol alphabet.

Custom-drawn number keys and the symbol grid redraw when their bounds change.
This prevents the initial extension height adjustment from stretching cached
glyphs until the first touch.

## Run and install

Open `TwoABC.xcodeproj`, choose the **TwoABC** scheme and your iPhone. Set your
development team for **both TwoABC and TwoABCKeyboard** under Signing &
Capabilities, then Run. The app and embedded extension install together. This
uses a new bundle ID (`com.moshi.twoabc`) and does not replace Speak Pocket.

Once Xcode is signed into your Apple account, the CLI can also sign, install,
and launch it:

```sh
APPLE_TEAM_ID=YOUR_TEAM_ID IOS_DEVICE_ID=YOUR_DEVICE_ID ./keyboard.sh deploy
```

Use `xcrun devicectl list devices` to find the device ID. Xcode needs to create
separate provisioning profiles for the containing app and keyboard extension;
a cached profile for Speak Pocket cannot sign them. Keep the phone unlocked
during installation.

The practice app immediately presents the same keyboard view used by the
extension. Its Speak/Stop buttons use Apple's speech synthesis for a quick test.
The Keyboard button switches between the embedded practice keyboard and the
system's selected keyboard; the practice globe does the same.

Enable the extension in **Settings → General → Keyboard → Keyboards → Add New
Keyboard → 2ABC** (the containing app may be listed as 2ABC Practice). In any
eligible app, hold the globe key and choose 2ABC. Optional Full Access is requested
for haptic feedback. It also enlarges the keyboard's sandbox, including allowing
network access. This implementation contains no network calls, analytics,
speech model, or keystroke storage.
Speak Pocket continues to handle speech when this keyboard is selected there.

iOS temporarily uses its own keyboard for secure fields and phone pads. Apps
can also disallow third-party keyboards. See Apple's
[custom keyboard guide](https://developer.apple.com/library/archive/documentation/General/Conceptual/ExtensibilityPG/CustomKeyboard.html).

## Checks

From the workspace root:

```sh
./keyboard.sh check       # pure Swift mapping and gesture checks
./keyboard.sh simulator   # app + extension for Simulator
./keyboard.sh build       # unsigned iPhone app + extension
xcrun simctl list devices available
IOS_SIMULATOR_ID=YOUR_SIMULATOR_UDID ./keyboard.sh test
```

Build outputs are in `keyboard/build/Build/Products/`. Simulator UI tests use
real touch gestures in the practice app and verify text insertion, number taps,
four-letter keys, one-shot shift, deletion, cursor editing, long swipes,
symbol-page taps/deletion/return, period-to-symbol dragging, and holding space
without switching the layout. Simulator tests cannot establish
one-handed comfort or haptic reliability on an iPhone.

The system-extension test also enables 2ABC in the selected simulator's Settings,
selects it from the system keyboard picker, verifies text insertion and held
space and symbols through `UITextDocumentProxy`, compares an untouched key's
screenshot with its appearance after a tap to catch initially stretched glyphs,
and switches away using the globe. The mapping checks, six practice tests,
and system-extension test passed on the
iOS 26.1 simulator.

The Release app and embedded extension were also signed and installed
on the connected iPhone 15 Pro Max. The user confirmed that the initial glyph
distortion is fixed and that enabling Full Access restores extension haptics.

For a first phone trial, swipe `4↑ 3↑ 5→ 5→ 6→` to type `hello`, then try
the four-letter keys, number taps, backspace, and holding space. Select the
extension in Speak Pocket and verify typing while speech is playing.
