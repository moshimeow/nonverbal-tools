"""Stage the Dioxus device app, native runtime, and offline model for signing."""
from pathlib import Path
import plistlib
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]
source = ROOT / "target/dx/speak-pocket/release/ios/SpeakPocket.app"
if not (source / "Info.plist").is_file():
    raise SystemExit(f"Missing Dioxus iPhone bundle: {source}")
app = ROOT / "dist/SpeakPocket.app"
# Only replace our generated staging output, never source files or model inputs.
if app.exists():
    shutil.rmtree(app)
shutil.copytree(source, app, symlinks=True)
frameworks = list((ROOT / "target").glob("**/ios-arm64/SherpaOnnxC.framework"))
if not frameworks:
    raise SystemExit("Missing SherpaOnnxC iPhone framework; complete the iOS build first")
shutil.copytree(frameworks[0], app / "Frameworks/SherpaOnnxC.framework", dirs_exist_ok=True)
model = ROOT / "assets/kokoro-multi-lang-v1_0"
dest = app / "kokoro"
dest.mkdir(exist_ok=True)
for name in ["model.onnx", "voices.bin", "tokens.txt", "lexicon-gb-en.txt", "lexicon-zh.txt", "number-zh.fst", "LICENSE", "README.md"]:
    shutil.copy2(model / name, dest / name)
shutil.copytree(model / "dict", dest / "dict", dirs_exist_ok=True)
shutil.copytree(ROOT / "licenses", app / "licenses", dirs_exist_ok=True)
with (app / "Info.plist").open("rb") as f:
    info = plistlib.load(f)
info.update({
    "CFBundleIdentifier": "com.moshi.speakpocket",
    "CFBundleDisplayName": "Speak Pocket",
    "CFBundleName": "SpeakPocket",
    "CFBundleShortVersionString": "0.1.0",
    "CFBundleVersion": "1",
    "MinimumOSVersion": "18.0",
    "ITSAppUsesNonExemptEncryption": False,
    "NSLocationWhenInUseUsageDescription": "Save where you sent each Speak or Write message in your private history.",
})
with (app / "Info.plist").open("wb") as f:
    plistlib.dump(info, f)
binary = app / info["CFBundleExecutable"]
subprocess.run(["xcrun", "lipo", str(binary), "-verify_arch", "arm64"], check=True)
print(f"Unsigned device app: {app}")
print("Install with ./ios.sh deploy after configuring Xcode signing.")
