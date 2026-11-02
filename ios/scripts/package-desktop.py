"""Package the identical UI and offline model as a relocatable macOS app."""
from pathlib import Path
import plistlib
import shutil
import subprocess

root = Path(__file__).resolve().parents[1]
source = root / "target/dx/speak-pocket/release/macos/SpeakPocket.app"
app = root / "dist/macos/Speak Pocket.app"
if not (source / "Contents/Info.plist").is_file():
    raise SystemExit("Build the desktop app first")
if app.exists():
    shutil.rmtree(app)  # Generated staging output only.
shutil.copytree(source, app, symlinks=True)
resources = app / "Contents/Resources"
model = root / "assets/kokoro-multi-lang-v1_0"
dest = resources / "kokoro"
dest.mkdir(parents=True, exist_ok=True)
for name in ["model.onnx", "voices.bin", "tokens.txt", "lexicon-gb-en.txt", "lexicon-zh.txt", "number-zh.fst", "LICENSE", "README.md"]:
    shutil.copy2(model / name, dest / name)
shutil.copytree(model / "dict", dest / "dict", dirs_exist_ok=True)
shutil.copytree(root / "licenses", resources / "licenses", dirs_exist_ok=True)
plist = app / "Contents/Info.plist"
with plist.open("rb") as f:
    info = plistlib.load(f)
info.update({"CFBundleDisplayName": "Speak Pocket", "CFBundleName": "Speak Pocket",
             "NSLocationUsageDescription": "Save where you sent each message in your private history.",
             "NSLocationWhenInUseUsageDescription": "Save where you sent each message in your private history."})
with plist.open("wb") as f:
    plistlib.dump(info, f)
# Ad-hoc signing is sufficient for this locally built app on this Mac.
subprocess.run(["codesign", "--force", "--deep", "--sign", "-", str(app)], check=True)
subprocess.run(["codesign", "--verify", "--deep", "--strict", str(app)], check=True)
print(f"Desktop app: {app}")
