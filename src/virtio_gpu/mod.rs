pub mod device {
    include!(concat!(env!("OUT_DIR"), "/device.rs"));
}

mod device_ext;

pub use device::VirtioGpuDevice;

pub mod display;
pub mod features;
pub mod protocol;
pub mod renderer;
pub mod resource;
pub mod transport;
pub mod venus;
