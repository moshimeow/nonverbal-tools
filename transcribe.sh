#!/bin/bash
# Local OpenAI Whisper; kept separate from the TTS Python environment.
set -euo pipefail
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
PYTHON="$ROOT/.tools/whisper-env/bin/python"

if [ "${1:-}" = "--setup" ]; then
  BOOTSTRAP="$ROOT/.conda/bin/python"
  if [ ! -x "$BOOTSTRAP" ]; then BOOTSTRAP=python3; fi
  "$BOOTSTRAP" -m venv "$ROOT/.tools/whisper-env"
  "$PYTHON" -m pip install 'openai-whisper==20250625'
  "$PYTHON" -c 'import sys, whisper; whisper.load_model("base.en", device="cpu", download_root=sys.argv[1])' "$ROOT/.tools/whisper-models"
  exit 0
fi

if [ "$#" -eq 0 ]; then
  echo 'Usage: ./transcribe.sh --setup | AUDIO_OR_VIDEO [Whisper options]'
  exit 0
fi
if [ ! -x "$PYTHON" ]; then
  echo 'Run ./transcribe.sh --setup first.' >&2
  exit 1
fi
command -v ffmpeg >/dev/null || { echo 'FFmpeg is required.' >&2; exit 1; }
exec "$PYTHON" -m whisper --model base.en --device cpu --fp16 False \
  --model_dir "$ROOT/.tools/whisper-models" \
  --output_dir "$ROOT/transcriptions" --output_format all "$@"
