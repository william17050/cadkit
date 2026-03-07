#!/usr/bin/env sh
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
cd "$SCRIPT_DIR"

WGPU_BACKEND=${WGPU_BACKEND:-gl}
WINIT_UNIX_BACKEND=${WINIT_UNIX_BACKEND:-x11}

export WGPU_BACKEND
export WINIT_UNIX_BACKEND

if [ "${1-}" = "--py" ]; then
  shift
  PYO3_PYTHON=${PYO3_PYTHON:-/usr/bin/python3}
  export PYO3_PYTHON
  exec cargo run -p cadkit --features python-scripting "$@"
fi

exec cargo run -p cadkit "$@"
