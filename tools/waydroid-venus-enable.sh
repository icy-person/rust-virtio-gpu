#!/usr/bin/env bash
set -euo pipefail

# Configure the host-side Venus/vtest connection for Waydroid.
# Run while Waydroid is stopped. Android system files are only changed when an
# init.environ.rc overlay is found or explicitly supplied.

ROOT=/var/lib/waydroid
HOST_SOCKET=${WAYDROID_VENUS_SOCKET:-/tmp/.virgl_test}
GUEST_SOCKET=${WAYDROID_VENUS_GUEST_SOCKET:-/run/xdg/.virgl_test}
CONFIG=${WAYDROID_VENUS_CONFIG:-$ROOT/config_session}
PROP=${WAYDROID_VENUS_PROP:-$ROOT/waydroid.prop}
INIT_ENV=${WAYDROID_VENUS_INIT_ENV:-}
SERVER=${VIRGL_TEST_SERVER:-virgl_test_server}
ICD=${WAYDROID_VENUS_ICD:-/vendor/etc/vulkan/icd.d/virtio_icd.x86_64.json}
RENDER_NODE=${WAYDROID_VENUS_RENDER_NODE:-}

if waydroid status 2>/dev/null | grep -q "RUNNING"; then
    echo "Stop Waydroid first: sudo waydroid session stop" >&2
    exit 1
fi

command -v "$SERVER" >/dev/null 2>&1 || {
    echo "virgl_test_server not found: $SERVER" >&2
    exit 1
}

mkdir -p "$(dirname "$CONFIG")"
touch "$CONFIG"
ENTRY="lxc.mount.entry = $HOST_SOCKET ${GUEST_SOCKET#/} none bind,create=file,optional 0 0"
if ! grep -Fqx "$ENTRY" "$CONFIG"; then
    printf '\n%s\n' "$ENTRY" >> "$CONFIG"
fi

# Waydroid binds this file to vendor/waydroid.prop during container startup.
mkdir -p "$(dirname "$PROP")"
touch "$PROP"
set_prop() {
    local key=$1 value=$2 tmp
    tmp=$(mktemp)
    awk -v key="$key" -v value="$value" '
        index($0, key "=") == 1 { if (!seen) { print key "=" value; seen=1 } next }
        { print }
        END { if (!seen) print key "=" value }
    ' "$PROP" > "$tmp"
    cat "$tmp" > "$PROP"
    rm -f "$tmp"
}
set_prop ro.hardware.vulkan virtio
set_prop ro.hardware.egl mesa
set_prop ro.hardware.gralloc gbm

if [[ -z "$INIT_ENV" ]]; then
    for candidate in \
        "$ROOT/overlay/init.environ.rc" \
        "$ROOT/overlay/system/etc/init.environ.rc" \
        "$ROOT/rootfs/init.environ.rc"; do
        if [[ -f "$candidate" ]]; then
            INIT_ENV=$candidate
            break
        fi
    done
fi

if [[ -n "$INIT_ENV" ]]; then
    for kv in \
        "VN_DEBUG vtest" \
        "VTEST_SOCKET_NAME $GUEST_SOCKET" \
        "VK_DRIVER_FILES $ICD" \
        "GALLIUM_DRIVER virpipe" \
        "LIBVA_DRIVER_NAME virtio_gpu"; do
        name=${kv%% *}
        value=${kv#* }
        grep -Fqx "    export $name $value" "$INIT_ENV" 2>/dev/null || printf '\n    export %s %s\n' "$name" "$value" >> "$INIT_ENV"
    done
    echo "Patched Android init environment: $INIT_ENV"
else
    echo "No init.environ.rc overlay found; pass WAYDROID_VENUS_INIT_ENV=/path/to/init.environ.rc to patch it."
fi

echo "Configured VirtIO/Venus socket bind: $HOST_SOCKET -> $GUEST_SOCKET"
echo "Android Vulkan ICD: $ICD"
if [[ -n "$RENDER_NODE" ]]; then
    echo "Host render node: $RENDER_NODE"
fi

echo
echo "Start the host Venus server before the Waydroid session:"
echo "  virgl_test_server --venus --no-fork --multi-clients --use-egl-surfaceless"
echo "Then start Waydroid: sudo waydroid session start"
