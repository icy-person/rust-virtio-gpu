use ash::{Device, vk};

pub struct DescriptorPool {
    pub pool: vk::DescriptorPool,
}

impl DescriptorPool {
    pub fn new(device: &Device) -> Result<Self, vk::Result> {
        let pool_size = vk::DescriptorPoolSize::default()
            .ty(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .descriptor_count(1);

        let info = vk::DescriptorPoolCreateInfo::default()
            .pool_sizes(std::slice::from_ref(&pool_size))
            .max_sets(1);

        let pool = unsafe { device.create_descriptor_pool(&info, None)? };

        println!("Descriptor Pool created.");

        Ok(Self { pool })
    }
}
