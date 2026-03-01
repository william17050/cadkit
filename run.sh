#!/usr/bin/env sh
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
cd "$SCRIPT_DIR"

WGPU_BACKEND=${WGPU_BACKEND:-gl}
WINIT_UNIX_BACKEND=${WINIT_UNIX_BACKEND:-x11}

export WGPU_BACKEND
export WINIT_UNIX_BACKEND

exec cargo run -p cadkit "$@"
