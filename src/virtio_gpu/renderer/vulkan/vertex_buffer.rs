use ash::{Device, Instance, vk};

use super::{Buffer, DeviceMemory, Vertex};

pub struct VertexBuffer {
    pub buffer: Buffer,
    pub memory: DeviceMemory,
    pub vertex_count: u32,
}

impl VertexBuffer {
    pub fn new(
        instance: &Instance,
        physical_device: vk::PhysicalDevice,
        device: &Device,
        vertices: &[Vertex],
    ) -> Result<Self, vk::Result> {
        let size = (vertices.len() * std::mem::size_of::<Vertex>()) as vk::DeviceSize;

        let buffer = Buffer::new(device, size, vk::BufferUsageFlags::VERTEX_BUFFER)?;

        let memory = DeviceMemory::allocate_buffer(
            instance,
            physical_device,
            device,
            buffer.buffer,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        )?;

        unsafe {
            device.bind_buffer_memory(buffer.buffer, memory.memory, 0)?;

            let ptr = device.map_memory(memory.memory, 0, size, vk::MemoryMapFlags::empty())?;

            std::ptr::copy_nonoverlapping(vertices.as_ptr(), ptr.cast(), vertices.len());

            device.unmap_memory(memory.memory);
        }

        Ok(Self {
            buffer,

            memory,

            vertex_count: vertices.len() as u32,
        })
    }

    pub fn bind(&self, device: &Device, cmd: vk::CommandBuffer) {
        unsafe {
            device.cmd_bind_vertex_buffers(cmd, 0, std::slice::from_ref(&self.buffer.buffer), &[0]);
        }
    }
}
