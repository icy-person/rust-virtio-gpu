# Waydroid gaming with Mesa Venus

This repository contains the VirtIO-GPU protocol, resource/blob state, host-side Venus/virglrenderer integration, and a small host launcher for the Waydroid vtest path.

CI validation is currently focused on the Rust build, launcher build, tests, and Clippy; hardware Waydroid/Vulkan validation still requires a real Android container and host GPU.

The current CI branch also verifies that the public Venus runtime wrapper owns backend lifecycle operations such as resource backing detach, so device extensions never reach into wrapper internals.