use winit::{
    event_loop::EventLoop,
    window::{Window, WindowAttributes},
};

pub struct DisplayWindow {
    pub window: Window,
}

#[allow(deprecated)]
impl DisplayWindow {
    pub fn new(event_loop: &EventLoop<()>) -> Self {
        let window = event_loop
            .create_window(
                WindowAttributes::default()
                    .with_title("VirtIO-GPU WGPU")
                    .with_inner_size(winit::dpi::LogicalSize::new(1920, 1080)),
            )
            .unwrap();

        Self { window }
    }
}
