use super::framebuffer::FrameBuffer;
use super::renderer::Renderer;
use crate::virtio_gpu::device::DeviceError;
use crate::virtio_gpu::resource::Resource;

#[derive(Debug)]
pub struct SoftwareRenderer {
    framebuffer: FrameBuffer,
}

impl SoftwareRenderer {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            framebuffer: FrameBuffer::new(width, height),
        }
    }

    pub fn upload(&mut self, data: &[u8]) {
        self.framebuffer.update(data);
    }

    pub fn pixels(&self) -> &[u8] {
        &self.framebuffer.data
    }

    pub fn size(&self) -> (u32, u32) {
        (self.framebuffer.width, self.framebuffer.height)
    }
    pub fn framebuffer(&self) -> &FrameBuffer {
        &self.framebuffer
    }
    pub fn framebuffer_mut(&mut self) -> &mut FrameBuffer {
        &mut self.framebuffer
    }
}

impl Renderer for SoftwareRenderer {
    fn upload(&mut self, data: &[u8]) {
        self.upload(data);
    }
    fn framebuffer(&self) -> &FrameBuffer {
        self.framebuffer()
    }
    fn framebuffer_mut(&mut self) -> &mut FrameBuffer {
        self.framebuffer_mut()
    }

    fn transfer_resource(&mut self, resource: &mut Resource) -> Result<(), DeviceError> {
        self.framebuffer = FrameBuffer::new(resource.width, resource.height);
        self.upload(resource.pixels());

        Ok(())
    }
    fn flush_resource(&mut self, resource: &mut Resource) -> Result<(), DeviceError> {
        self.framebuffer = FrameBuffer::new(resource.width, resource.height);
        self.upload(resource.pixels());

        Ok(())
    }
}
