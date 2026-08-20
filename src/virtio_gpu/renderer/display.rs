use minifb::{Key, Window, WindowOptions};

use super::framebuffer::FrameBuffer;

pub struct Display {
    window: Option<Window>,
}

impl Display {
    pub fn new(width: usize, height: usize) -> Self {
        let window = Window::new("rust-virtio-gpu", width, height, WindowOptions::default()).ok();
        Self { window }
    }

    pub fn update(&mut self, framebuffer: &mut FrameBuffer) {
        let Some(window) = self.window.as_mut() else {
            return;
        };

        let pixels: Vec<u32> = framebuffer
            .data
            .chunks_exact(4)
            .map(|p| {
                let b = p[0] as u32;
                let g = p[1] as u32;
                let r = p[2] as u32;
                (r << 16) | (g << 8) | b
            })
            .collect();

        let _ = window.update_with_buffer(
            &pixels,
            framebuffer.width as usize,
            framebuffer.height as usize,
        );
    }

    pub fn is_open(&self) -> bool {
        self.window
            .as_ref()
            .is_some_and(|window| window.is_open() && !window.is_key_down(Key::Escape))
    }

    pub fn is_headless(&self) -> bool {
        self.window.is_none()
    }
}
