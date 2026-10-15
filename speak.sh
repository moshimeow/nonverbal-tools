#!/bin/sh
# Use the project's Python even when conda base is active.
set -eu
HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
if [ ! -x "$HERE/.conda/bin/python" ]; then
    echo "Create the environment first: conda env create --prefix '$HERE/.conda' --file '$HERE/environment.yml'" >&2
    exit 1
fi
exec "$HERE/.conda/bin/python" "$HERE/speak.py" "$@"
