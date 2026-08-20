use ash::{Device, vk};

use super::{DescriptorPool, DescriptorSetLayout};

pub struct DescriptorSet {
    pub set: vk::DescriptorSet,
}

impl DescriptorSet {
    pub fn new(
        device: &Device,
        layout: &DescriptorSetLayout,
        pool: &DescriptorPool,
    ) -> Result<Self, vk::Result> {
        let layouts = [layout.layout];

        let info = vk::DescriptorSetAllocateInfo::default()
            .descriptor_pool(pool.pool)
            .set_layouts(&layouts);

        let set = unsafe { device.allocate_descriptor_sets(&info)? }[0];

        println!("Descriptor Set allocated.");

        Ok(Self { set })
    }
}
