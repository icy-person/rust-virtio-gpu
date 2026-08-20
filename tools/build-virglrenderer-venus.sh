#!/usr/bin/env bash
set -euo pipefail

ROOT=${1:-"$PWD/third_party/virglrenderer"}
REPO=${VIRGLRENDERER_REPO:-https://gitlab.freedesktop.org/virgl/virglrenderer.git}
BUILD="$ROOT/out"

if [[ ! -d "$ROOT/.git" ]]; then
    mkdir -p "$(dirname "$ROOT")"
    git clone --depth=1 "$REPO" "$ROOT"
fi

meson setup "$BUILD" "$ROOT" -Dvenus=true --buildtype=release || \
    meson setup --reconfigure "$BUILD" "$ROOT" -Dvenus=true --buildtype=release
meson compile -C "$BUILD"

SERVER="$BUILD/vtest/virgl_test_server"
if [[ ! -x "$SERVER" ]]; then
    echo "virgl_test_server was not produced at $SERVER" >&2
    exit 1
fi

"$SERVER" --help | grep -q -- '--venus'
printf 'Venus vtest server: %s\n' "$SERVER"
