# Waydroid gaming with Mesa Venus

This repository contains the VirtIO-GPU protocol, resource/blob state, host-side Venus/virglrenderer integration, and a small host launcher for the Waydroid vtest path.

## Architecture

For Waydroid, use the following path:

```text
Android app
   |
   | Vulkan
   v
Mesa Android virtio Vulkan driver
   |
   | Venus / vtest
   v
Unix socket: /run/xdg/.virgl_test
   |
   | bind mount from host
   v
Unix socket: /tmp/.virgl_test
   |
virgl_test_server --venus
   |
   v
virglrenderer Venus backend
   |
   v
host Vulkan driver / render node
```

Waydroid is a container, so this path is deliberately different from the PCI transport used by a VM. The PCI implementation remains useful for QEMU/crosvm-style guests; the Waydroid integration uses the host-side vtest Venus server.

## Host requirements

The host must have a Mesa/virglrenderer build that exposes `virgl_test_server --venus`, and the Vulkan stack must support the external-memory and DRM-modifier capabilities required by Venus. A discrete or integrated GPU should be selected with `--rendernode` when the machine has more than one render device.

For a host Vulkan ICD that is not the default loader choice:

```bash
waydroid-venus \
  --host-icd /usr/share/vulkan/icd.d/<host-icd>.json \
  --rendernode /dev/dri/renderD128 \
  --setup \
  --start
```

`--host-icd` is for the **host** Vulkan loader. `--icd` is for the **Android guest** Vulkan ICD and normally points at:

```text
/vendor/etc/vulkan/icd.d/virtio_icd.x86_64.json
```

Do not confuse these two paths.

## Build this project

```bash
cargo build --release --all-features
```

The helper executable is:

```text
target/release/waydroid-venus
```

The virglrenderer/Venus backend is enabled with:

```bash
cargo build --release --all-features
```

## Configure Waydroid

Stop the container first:

```bash
sudo waydroid session stop || true
sudo waydroid container stop || true
```

Apply the socket bind:

```bash
sudo target/release/waydroid-venus --setup
```

The helper adds this idempotent LXC entry to `config_session`:

```text
lxc.mount.entry = /tmp/.virgl_test run/xdg/.virgl_test none bind,create=file,optional 0 0
```

The host socket must exist **before** the Waydroid container starts.

## Android environment

Mesa's vtest Venus transport needs the following process environment in Android:

```text
VN_DEBUG=vtest
VTEST_SOCKET_NAME=/run/xdg/.virgl_test
VK_DRIVER_FILES=/vendor/etc/vulkan/icd.d/virtio_icd.x86_64.json
```

The helper can update an `init.environ.rc` overlay passed explicitly with `--init-env`. It does not overwrite an Android system image automatically.

The generated environment also includes the legacy GL/VA-API path used by virpipe/virtio-gpu:

```text
LIBGL_ALWAYS_SOFTWARE=1
GALLIUM_DRIVER=virpipe
LIBVA_DRIVER_NAME=virtio_gpu
```

## Android properties

For a Mesa virtio/GBM stack, apply these properties to the Waydroid overlay/prop file you use for your image:

```text
ro.hardware.vulkan=virtio
ro.hardware.egl=mesa
ro.hardware.gralloc=gbm
```

The Rust helper can apply them to files supplied through `--prop`.

## Start sequence

A reliable startup order is:

1. Stop Waydroid.
2. Configure the LXC socket bind.
3. Start `virgl_test_server` with Venus enabled.
4. Confirm `/tmp/.virgl_test` exists.
5. Start the Waydroid session.
6. Verify the Android Mesa virtio ICD is present and `VTEST_SOCKET_NAME` is visible in the Android process environment.

The helper can perform steps 2-4:

```bash
sudo target/release/waydroid-venus --setup --start
```

Leave it running and start Waydroid from another terminal.

## Performance settings for games

For Vulkan games, prefer Venus and avoid verbose debug output. Use a real render node rather than a generic software Vulkan ICD when available:

```bash
waydroid-venus \
  --rendernode /dev/dri/renderD128 \
  --host-icd /usr/share/vulkan/icd.d/<real-gpu>.json \
  --setup --start
```

Do not use `LIBGL_ALWAYS_SOFTWARE=1` as a way to select Vulkan; that variable is for the GL/virpipe side. Vulkan selection is controlled by the Android virtio ICD plus the Venus vtest transport.

For stable gaming behavior, keep shader cache enabled and do not enable `VN_DEBUG` modes other than the minimal `vtest` transport selection in production.

## Validation

Host:

```bash
virgl_test_server --help | grep -- --venus
ls -l /tmp/.virgl_test
vulkaninfo --summary
```

Waydroid:

```bash
waydroid shell getprop ro.hardware.vulkan
waydroid shell ls -l /vendor/etc/vulkan/icd.d/
waydroid shell sh -c 'cat /proc/1/environ | tr "\\0" "\\n" | grep -E "VN_DEBUG|VTEST_SOCKET_NAME|VK_DRIVER_FILES"'
```

The important end-to-end property is that a Vulkan application inside Android creates a `virtio` Vulkan device and the command stream reaches the host Venus server rather than falling back to SwiftShader.

## Troubleshooting

### SwiftShader is selected

Check the Android virtio ICD exists and that `VK_DRIVER_FILES` points to it. Also verify that the Android image actually contains Mesa's virtio Vulkan driver.

### `VTEST_SOCKET_NAME` is set but connection fails

Check the bind mount from `/tmp/.virgl_test` to `/run/xdg/.virgl_test`, and make sure the host socket exists before `lxc-start`.

### Server starts but Vulkan fails

Verify the host Vulkan driver and external-memory/DRM-modifier requirements. Try selecting the correct host render node explicitly.

### Performance is still poor

Check that the host render node is the real GPU, not `llvmpipe`/software Vulkan. Venus cannot turn an unsupported host Vulkan stack into a hardware Vulkan backend.

## Scope

The PCI transport in this project targets VM-style VirtIO-GPU devices. The Waydroid integration intentionally uses the vtest Unix-socket path instead of pretending Waydroid has a PCI device. This distinction is required for a real container deployment.
