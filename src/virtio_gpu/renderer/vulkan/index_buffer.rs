use ash::{Device, Instance, vk};

use super::{Buffer, DeviceMemory};

pub struct IndexBuffer {
    pub buffer: Buffer,
    pub memory: DeviceMemory,
    pub index_count: u32,
}

impl IndexBuffer {
    pub fn new(
        instance: &Instance,
        physical_device: vk::PhysicalDevice,
        device: &Device,
        indices: &[u32],
    ) -> Result<Self, vk::Result> {
        let size = (indices.len() * std::mem::size_of::<u32>()) as vk::DeviceSize;

        let buffer = Buffer::new(device, size, vk::BufferUsageFlags::INDEX_BUFFER)?;

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

            std::ptr::copy_nonoverlapping(indices.as_ptr(), ptr.cast(), indices.len());

            device.unmap_memory(memory.memory);
        }

        Ok(Self {
            buffer,

            memory,

            index_count: indices.len() as u32,
        })
    }

    pub fn bind(&self, device: &Device, cmd: vk::CommandBuffer) {
        unsafe {
            device.cmd_bind_index_buffer(cmd, self.buffer.buffer, 0, vk::IndexType::UINT32);
        }
    }
}
