use ash::{Device, Instance, vk};

pub struct DeviceMemory {
    pub memory: vk::DeviceMemory,
}

impl DeviceMemory {
    pub fn allocate_buffer(
        instance: &Instance,
        physical_device: vk::PhysicalDevice,
        device: &Device,
        buffer: vk::Buffer,
        required: vk::MemoryPropertyFlags,
    ) -> Result<Self, vk::Result> {
        let requirements = unsafe { device.get_buffer_memory_requirements(buffer) };

        let properties = unsafe { instance.get_physical_device_memory_properties(physical_device) };

        let mut memory_index = None;

        for i in 0..properties.memory_type_count {
            let supported = requirements.memory_type_bits & (1 << i) != 0;

            let flags = properties.memory_types[i as usize].property_flags;

            if supported && flags.contains(required) {
                memory_index = Some(i);

                break;
            }
        }

        let memory_index = memory_index.expect("No suitable buffer memory");

        let info = vk::MemoryAllocateInfo::default()
            .allocation_size(requirements.size)
            .memory_type_index(memory_index);

        let memory = unsafe { device.allocate_memory(&info, None)? };

        Ok(Self { memory })
    }

    pub fn allocate_image(
        instance: &Instance,
        physical_device: vk::PhysicalDevice,
        device: &Device,
        image: vk::Image,
        required: vk::MemoryPropertyFlags,
    ) -> Result<Self, vk::Result> {
        let requirements = unsafe { device.get_image_memory_requirements(image) };

        let properties = unsafe { instance.get_physical_device_memory_properties(physical_device) };

        let mut memory_index = None;

        for i in 0..properties.memory_type_count {
            let supported = requirements.memory_type_bits & (1 << i) != 0;

            let flags = properties.memory_types[i as usize].property_flags;

            if supported && flags.contains(required) {
                memory_index = Some(i);

                break;
            }
        }

        let memory_index = memory_index.expect("No suitable image memory");

        let info = vk::MemoryAllocateInfo::default()
            .allocation_size(requirements.size)
            .memory_type_index(memory_index);

        let memory = unsafe { device.allocate_memory(&info, None)? };

        Ok(Self { memory })
    }
}
