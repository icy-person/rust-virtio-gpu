pub mod device {
    include!(concat!(env!("OUT_DIR"), "/device.rs"));
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/virtio_gpu/device_ext.rs"
    ));
}

pub use device::VirtioGpuDevice;

pub mod display;
pub mod features;
pub mod protocol;
pub mod renderer;
pub mod resource;
pub mod transport;
pub mod venus;
