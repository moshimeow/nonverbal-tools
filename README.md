# Type-to-speak with Kokoro

Local neural TTS on an Apple Silicon Mac, using MLX Audio and Kokoro.

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
The environment installs `misaki[en]`, including phonemizer and bundled eSpeak.
See [Misaki's English setup](https://github.com/hexgrad/misaki#english-usage).

## Use

```sh
./speak.sh --voice af_heart
./speak.sh --list-devices
./speak.sh --device 'BlackHole'
```

Type a line and press Enter to speak. Commands:

- `/voice af_bella`: change voice and hear a sample.
- `/speed 1.1`: change speed (0.5 to 2.0).
- `/again`: repeat the last line.
- `:p`: list phrases from `phrases.txt`.
- `:1`, `:2`, etc.: speak a saved phrase.
- `/quit` or Ctrl-D: exit.

For British English, use e.g. `./speak.sh --lang b --voice bf_emma`.
