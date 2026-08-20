#!/usr/bin/env bash
set -euo pipefail

HOST_SOCKET=${WAYDROID_VENUS_SOCKET:-/tmp/.virgl_test}
GUEST_SOCKET=${WAYDROID_VENUS_GUEST_SOCKET:-/run/xdg/.virgl_test}
ICD=${WAYDROID_VENUS_ICD:-/vendor/etc/vulkan/icd.d/virtio_icd.x86_64.json}
RENDER_NODE=${WAYDROID_VENUS_RENDER_NODE:-}

fail=0
check() {
    local label=$1
    shift
    if "$@"; then
        printf '[OK] %s\n' "$label"
    else
        printf '[FAIL] %s\n' "$label"
        fail=1
    fi
}

check "virgl_test_server supports Venus" bash -c 'command -v virgl_test_server >/dev/null && virgl_test_server --help 2>&1 | grep -q -- "--venus"'
check "host vtest socket exists" test -S "$HOST_SOCKET"
if [[ -n "$RENDER_NODE" ]]; then
    check "render node exists" test -e "$RENDER_NODE"
fi

if command -v vulkaninfo >/dev/null 2>&1; then
    printf '\nHost Vulkan:\n'
    vulkaninfo --summary 2>/dev/null | sed -n '1,24p' || true
fi

if command -v waydroid >/dev/null 2>&1; then
    printf '\nWaydroid:\n'
    waydroid status || true
    waydroid shell getprop ro.hardware.vulkan || true
    waydroid shell getprop ro.hardware.egl || true
    waydroid shell getprop ro.hardware.gralloc || true
    waydroid shell sh -c "test -f '$ICD'" && echo "[OK] Android Vulkan ICD: $ICD" || {
        echo "[FAIL] Android Vulkan ICD missing: $ICD"
        fail=1
    }
    waydroid shell sh -c 'cat /proc/1/environ 2>/dev/null | tr "\\0" "\\n" | grep -E "^(VN_DEBUG|VTEST_SOCKET_NAME|VK_DRIVER_FILES)="' || true
fi

if [[ "$fail" -ne 0 ]]; then
    echo
    echo "Waydroid Venus health check failed. See docs/WAYDROID_GAMING.md."
    exit 1
fi

echo
printf 'Waydroid Venus health check passed. Guest socket: %s\n' "$GUEST_SOCKET"
