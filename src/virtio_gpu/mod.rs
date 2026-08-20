pub mod device {
    include!(concat!(env!("OUT_DIR"), "/device.rs"));
}
pub mod display;
pub mod features;
pub mod protocol;
pub mod renderer;
pub mod resource;
pub mod transport;
pub mod venus;
