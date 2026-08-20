# Completion status

## Verified in source

- VirtIO-GPU 2D request/response protocol and validation.
- Venus context/resource/blob request decoding.
- Host-side virglrenderer backend behind `virglrenderer-backend`.
- Guest-backed blob IOVecs with stable backing allocation.
- 3D resource creation and transfer routing.
- Context/resource attach and detach routing.
- Fence callback collection and guest-fence ID translation.
- Waydroid vtest launcher, socket setup, property/environment helpers.
- Linux x86_64 release packaging workflow.

## Still environment-dependent

- A GitHub runner cannot validate a real Android/Waydroid container or a physical GPU game.
- Hardware acceleration must be verified on the target host with the Android Mesa virtio/Venus ICD present.
- The Waydroid path uses Mesa's `virgl_test_server --venus`; this crate does not replace the Mesa vtest wire server.

## Release gate

A release must pass `fmt`, `check`, release build, launcher smoke test, unit/integration tests, and Clippy before assets are published.
