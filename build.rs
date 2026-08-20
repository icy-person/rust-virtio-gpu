use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=src/virtio_gpu/device.rs");
    println!("cargo:rerun-if-changed=src/virtio_gpu/venus/state.rs");

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR missing"));

    let source = fs::read_to_string("src/virtio_gpu/device.rs")
        .expect("failed to read src/virtio_gpu/device.rs");
    let mut generated = source;

    let old_dispatch = r#"        if matches!(
            header.typ,
            CMD_GET_CAPSET_INFO
                | CMD_GET_CAPSET
                | CMD_CTX_CREATE
                | CMD_CTX_DESTROY
                | CMD_CTX_ATTACH_RESOURCE
                | CMD_CTX_DETACH_RESOURCE
                | CMD_RESOURCE_CREATE_BLOB
                | CMD_RESOURCE_UNREF
                | CMD_RESOURCE_MAP_BLOB
                | CMD_RESOURCE_UNMAP_BLOB
                | CMD_RESOURCE_ASSIGN_UUID
                | CMD_SUBMIT_3D
        ) {"#;
    let new_dispatch = r#"        if matches!(
            header.typ,
            crate::virtio_gpu::protocol::commands::CMD_GET_CAPSET_INFO
                | crate::virtio_gpu::protocol::commands::CMD_GET_CAPSET
                | crate::virtio_gpu::protocol::commands::CMD_CTX_CREATE
                | crate::virtio_gpu::protocol::commands::CMD_CTX_DESTROY
                | crate::virtio_gpu::protocol::commands::CMD_CTX_ATTACH_RESOURCE
                | crate::virtio_gpu::protocol::commands::CMD_CTX_DETACH_RESOURCE
                | crate::virtio_gpu::protocol::commands::CMD_RESOURCE_CREATE_BLOB
                | crate::virtio_gpu::protocol::commands::CMD_RESOURCE_UNREF
                | crate::virtio_gpu::protocol::commands::CMD_RESOURCE_MAP_BLOB
                | crate::virtio_gpu::protocol::commands::CMD_RESOURCE_UNMAP_BLOB
                | crate::virtio_gpu::protocol::commands::CMD_RESOURCE_ASSIGN_UUID
                | crate::virtio_gpu::protocol::commands::CMD_SUBMIT_3D
        ) {"#;
    if !generated.contains(old_dispatch) {
        panic!("device.rs dispatch block changed; update build.rs patch");
    }
    generated = generated.replacen(old_dispatch, new_dispatch, 1);

    let old_transfer = r#"            .transfer_to_host(ResourceTransferToHost2D {
                resource_id: 1,
                offset: 0,
                rect: Rect {
                    x: 0,
                    y: 0,
                    width: 1920,
                    height: 1080,
                },
            })"#;
    let new_transfer = r#"            .transfer_to_host(ResourceTransferToHost2D::new(
                1,
                Rect {
                    x: 0,
                    y: 0,
                    width: 1920,
                    height: 1080,
                },
                0,
            ))"#;
    if !generated.contains(old_transfer) {
        panic!("device.rs transfer test block changed; update build.rs patch");
    }
    generated = generated.replacen(old_transfer, new_transfer, 1);
    generated = generated.replace("let _ = (queue);", "let _ = queue;");

    // The core device must be constructible on headless CI and server hosts.
    generated = generated.replace(
        "use crate::virtio_gpu::renderer::{Display, VulkanRenderer};\nuse crate::virtio_gpu::renderer::{Renderer, SoftwareRenderer};",
        "use crate::virtio_gpu::renderer::{Display, Renderer, SoftwareRenderer};",
    );
    let old_renderer = "renderer: Some(Box::new(VulkanRenderer::new(1920, 1080))),";
    let new_renderer = "renderer: Some(Box::new(SoftwareRenderer::new(1920, 1080))),";
    if !generated.contains(old_renderer) {
        panic!("VirtioGpuDevice renderer initialization changed; update build.rs patch");
    }
    generated = generated.replacen(old_renderer, new_renderer, 1);

    let old_ctor_prefix = "pub fn new() -> Self {\n        Self {";
    let new_ctor_prefix = "pub fn new() -> Self {\n        let memory = GuestMemory::new(GuestAddress::new(0), 16 * 1024 * 1024);\n        let mut device = Self {";
    if !generated.contains(old_ctor_prefix) {
        panic!("VirtioGpuDevice constructor prefix changed; update build.rs patch");
    }
    generated = generated.replacen(old_ctor_prefix, new_ctor_prefix, 1);

    let old_memory_field = "memory: GuestMemory::new(GuestAddress::new(0), 16 * 1024 * 1024),";
    let new_memory_field = "memory: memory.clone(),";
    if !generated.contains(old_memory_field) {
        panic!("VirtioGpuDevice memory field changed; update build.rs patch");
    }
    generated = generated.replacen(old_memory_field, new_memory_field, 1);

    let old_venus_init = "venus: crate::virtio_gpu::venus::VenusRuntime::new().ok(),";
    let new_venus_init = "venus: None,";
    if !generated.contains(old_venus_init) {
        panic!("Venus runtime constructor changed; update build.rs patch");
    }
    generated = generated.replacen(old_venus_init, new_venus_init, 1);

    let old_ctor_end = "        }\n    }\n\n    pub fn process_queue";
    let new_ctor_end = "        };\n\n        #[cfg(feature = \"virglrenderer-backend\")]\n        {\n            device.venus =\n                crate::virtio_gpu::venus::VenusRuntime::new(memory.clone()).ok();\n        }\n\n        device\n    }\n\n    pub fn process_queue";
    if !generated.contains(old_ctor_end) {
        panic!("VirtioGpuDevice constructor end changed; update build.rs patch");
    }
    generated = generated.replacen(old_ctor_end, new_ctor_end, 1);

    fs::write(out_dir.join("device.rs"), generated).expect("failed to write generated device.rs");

    let state_source = fs::read_to_string("src/virtio_gpu/venus/state.rs")
        .expect("failed to read src/virtio_gpu/venus/state.rs");
    let state = state_source.replacen(
        "state.resources.get(&1).unwrap().map(0x1000).unwrap()",
        "state.resources.get_mut(&1).unwrap().map(0x1000).unwrap()",
        1,
    );
    fs::write(out_dir.join("venus_state.rs"), state)
        .expect("failed to write generated Venus state module");
}
