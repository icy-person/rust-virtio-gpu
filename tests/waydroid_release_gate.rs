#[test]
fn release_gate_has_expected_launcher_and_docs() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    assert!(root.join("tools/waydroid-venus-enable.sh").is_file());
    assert!(root.join("tools/waydroid-venus-check.sh").is_file());
    assert!(root.join("tools/build-virglrenderer-venus.sh").is_file());
    assert!(root.join("tools/waydroid-venus.service").is_file());
    assert!(root.join("docs/WAYDROID_GAMING.md").is_file());
}
