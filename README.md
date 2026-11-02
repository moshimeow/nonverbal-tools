### iOS tools for if you can't talk
* SpeakPocket, entirely vibe coded by astra, little app that lets me do easy TTS or just show something to people in big text. Need to come up with a better name
* Clone of multilingual-o keyboard's [2ABC layout](keyboard/README.md), with a Swift iOS keyboard extension and practice app.

### Local transcription

Run OpenAI Whisper locally, separately from the TTS environment:

```sh
./transcribe.sh --setup
./transcribe.sh recording.mp4
```

Requires FFmpeg and Python 3.12 (setup uses the project's `.conda` Python when
available). Setup downloads Whisper and the English `base.en` model into
`.tools/`. Transcripts and subtitles are saved in `transcriptions/`; both
directories are ignored by Git. Recordings are processed locally, with no API
key or upload. Pass additional Whisper options after the filename, for example
`--model base --language Chinese` for a multilingual model. Selecting another
model downloads its weights on first use.
