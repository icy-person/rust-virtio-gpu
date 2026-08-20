# Completion gate

A Waydroid release is considered build-complete only when:

- `cargo fmt --all -- --check` passes.
- `cargo check --all-targets --all-features` passes.
- `cargo build --release --bin waydroid-venus --all-features` passes.
- `cargo test --all-targets --all-features` passes.
- `cargo clippy --all-targets --all-features -- -D warnings` passes.
- The Waydroid helper scripts pass `bash -n`.
- The release launcher responds to `--help`.
- A release archive and SHA256 checksum can be produced.

A hardware-gaming claim additionally requires a real Waydroid Android image with Mesa's virtio/Venus ICD and a real host Vulkan render node; GitHub-hosted CI cannot provide that end-to-end hardware test.
