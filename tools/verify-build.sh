#!/usr/bin/env bash
set -euo pipefail

cargo fmt --all -- --check
cargo check --all-targets --all-features
cargo build --release --bin waydroid-venus --all-features
./target/release/waydroid-venus --help >/dev/null
cargo test --all-targets --all-features
cargo clippy --all-targets --all-features -- -D warnings

mkdir -p release
install -m 0755 target/release/waydroid-venus release/waydroid-venus
sha256sum release/waydroid-venus > release/waydroid-venus.sha256
