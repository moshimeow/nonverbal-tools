#!/usr/bin/env python3
"""
speak.py: type a line, press Enter, Kokoro says it.

Commands inside the prompt:
  :1, :2 ...        speak saved phrase N from phrases.txt
  :p                list saved phrases
  /voice af_bella   switch voice
  /speed 1.1        change speed (0.5 to 2.0)
  /again            repeat the last thing you said
  /quit             exit (or Ctrl-D)
"""
import argparse
import os
import readline  # noqa: F401  (arrow keys + history in input())
import sys

import numpy as np
import sounddevice as sd
from mlx_audio.tts.utils import load_model

HERE = os.path.dirname(os.path.abspath(__file__))

p = argparse.ArgumentParser(description="Type-to-speak with Kokoro")
p.add_argument("--voice", default="af_heart", help="e.g. af_heart, af_bella, af_nova, bf_emma")
p.add_argument("--speed", type=float, default=1.0)
p.add_argument("--lang", default="a", help="a = American English, b = British English")
p.add_argument("--model", default="mlx-community/Kokoro-82M-bf16")
p.add_argument("--device", help="output device name or part of it (e.g. 'BlackHole', 'Multi-Output')")
p.add_argument("--phrases", default=os.path.join(HERE, "phrases.txt"))
p.add_argument("--list-devices", action="store_true", help="print audio devices and exit")
args = p.parse_args()

if args.list_devices:
    print(sd.query_devices())
    sys.exit()

device = None
if args.device:
    matches = [i for i, d in enumerate(sd.query_devices())
               if args.device.lower() in d["name"].lower() and d["max_output_channels"] > 0]
    if not matches:
        sys.exit(f"No output device matching '{args.device}'. Try --list-devices.")
    device = matches[0]

phrases = []
if os.path.exists(args.phrases):
    with open(args.phrases) as f:
        phrases = [line.strip() for line in f if line.strip() and not line.startswith("#")]

print("Loading Kokoro...", flush=True)
model = load_model(args.model)
voice, speed = args.voice, args.speed


def say(text):
    chunks, sr = [], 24000
    for r in model.generate(text=text, voice=voice, speed=speed, lang_code=args.lang):
        chunks.append(np.array(r.audio, dtype=np.float32))
        sr = getattr(r, "sample_rate", sr)
    if chunks:
        sd.play(np.concatenate(chunks), sr, device=device)
        sd.wait()


say("Ready.")  # warms up the model so the first real line is fast
print(f"Ready. Voice: {voice}, speed: {speed}. Type and press Enter. /quit to exit.")

last = ""
while True:
    try:
        line = input("> ").strip()
    except (EOFError, KeyboardInterrupt):
        print()
        break
    if not line:
        continue
    if line == "/quit":
        break
    if line == "/again":
        line = last
    elif line == ":p":
        for i, ph in enumerate(phrases, 1):
            print(f"  :{i}  {ph}")
        continue
    elif line.startswith(":") and line[1:].isdigit():
        n = int(line[1:])
        if 1 <= n <= len(phrases):
            line = phrases[n - 1]
        else:
            print("No phrase with that number.")
            continue
    elif line.startswith("/voice "):
        voice = line.split(maxsplit=1)[1]
        print(f"Voice: {voice}")
        say("This is my new voice.")
        continue
    elif line.startswith("/speed "):
        try:
            speed = max(0.5, min(2.0, float(line.split()[1])))
            print(f"Speed: {speed}")
        except ValueError:
            print("Usage: /speed 1.1")
        continue
    last = line
    try:
        say(line)
    except Exception as e:  # keep going if one line fails
        print(f"(couldn't speak that: {e})")