# Type-to-speak with Kokoro

Local neural TTS on an Apple Silicon Mac, using MLX Audio and Kokoro.

An offline Rust/Dioxus iPhone prototype lives in [`ios/`](ios/README.md).
Use `./ios.sh desktop` for the Mac UI, `./ios.sh check --play` to test the Rust
speech backend, or `./ios.sh build` to produce the iPhone app bundle.

## Setup

```sh
conda env create --prefix ./.conda --file environment.yml
./speak.sh
```

The launcher selects the project environment even if conda `base` is active.
Alternatively, run `conda activate ./.conda` followed by `python speak.py`.
The first run downloads model weights, voices, and the English spaCy model.
Later runs reuse the local cache. MLX needs access to the Mac GPU.

Python 3.12 is intentional: the published Misaki 0.9.4 package excludes Python
3.13. With Python 3.13, pip can select the older Misaki 0.7.4 instead. Installing
bare `misaki` also omits English dependencies; the resulting missing `phonemizer`
can be wrapped in a misleading "install misaki" error from KokoroPipeline.
The environment installs `misaki[en,zh]`, including English and Mandarin text processing,
phonemizer, and bundled eSpeak.
See [Misaki's English setup](https://github.com/hexgrad/misaki#english-usage).

## Use

```sh
./speak.sh --voice af_heart
./speak.sh --list-devices
./speak.sh --device 'BlackHole'
```

Type a line and press Enter to speak. Commands:

- `/voice af_bella`: change voice and its language, and hear a sample.
- `/speed 1.1`: change speed (0.5 to 2.0).
- `/again`: repeat the last line.
- `:p`: list phrases from `phrases.txt`.
- `:1`, `:2`, etc.: speak a saved phrase.
- `/quit` or Ctrl-D: exit.

For British English, use e.g. `./speak.sh --lang b --voice bf_emma`.

## Mandarin Chinese

```sh
./speak.sh --lang z
# Or choose a voice (the language is inferred):
./speak.sh --voice zf_xiaoxiao
```

Type Chinese characters, e.g. `你好！我现在可以用中文说话了。`
The default Mandarin voice is `zf_xiaoxiao`. Available feminine voices are
`zf_xiaobei`, `zf_xiaoni`, `zf_xiaoxiao`, and `zf_xiaoyi`; masculine voices are
`zm_yunjian`, `zm_yunxi`, `zm_yunxia`, and `zm_yunyang`.
See the [upstream voice list](https://huggingface.co/hexgrad/Kokoro-82M/blob/main/VOICES.md#mandarin-chinese).

Switch languages during a session with `/voice zf_xiaoxiao` or `/voice bf_emma`.
Language follows the selected voice, not the text; use the matching voice for
each language. Mixed English/Chinese pronunciation may be unreliable.

To add Mandarin dependencies to an existing installation:

```sh
conda env update --prefix ./.conda --file environment.yml
```
