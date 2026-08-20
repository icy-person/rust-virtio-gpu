#!/usr/bin/env bash
set -euo pipefail

# Configure a running Linux host for Mesa Venus in a Waydroid container.
# This helper deliberately does not modify Android system files automatically;
# use --init-env/--prop to point it at an overlay/custom image you control.

ROOT=/var/lib/waydroid
HOST_SOCKET=${WAYDROID_VENUS_SOCKET:-/tmp/.virgl_test}
GUEST_SOCKET=${WAYDROID_VENUS_GUEST_SOCKET:-/run/xdg/.virgl_test}
CONFIG=${WAYDROID_VENUS_CONFIG:-$ROOT/config_session}
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

echo "Configured VirtIO/Venus socket bind: $HOST_SOCKET -> $GUEST_SOCKET"
echo "Host Vulkan ICD expected inside Android: $ICD"
if [[ -n "$RENDER_NODE" ]]; then
    echo "Host render node: $RENDER_NODE"
fi

echo
cat <<'EOF'
Next steps:
  1. Start the host Venus server with:
       virgl_test_server --venus --no-fork --multi-clients --use-egl-surfaceless
  2. Make sure /tmp/.virgl_test exists before starting the Waydroid session.
  3. Export inside Android init:
       VN_DEBUG=vtest
       VTEST_SOCKET_NAME=/run/xdg/.virgl_test
       VK_DRIVER_FILES=/vendor/etc/vulkan/icd.d/virtio_icd.x86_64.json
       LIBGL_ALWAYS_SOFTWARE=1
       GALLIUM_DRIVER=virpipe
       LIBVA_DRIVER_NAME=virtio_gpu
  4. Ensure Android exposes the Mesa virtio Vulkan ICD and gralloc/egl are set to
       ro.hardware.vulkan=virtio
       ro.hardware.egl=mesa
       ro.hardware.gralloc=gbm
EOF
