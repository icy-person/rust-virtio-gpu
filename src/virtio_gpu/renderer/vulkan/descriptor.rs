use ash::{Device, vk};

pub struct DescriptorSetLayout {
    pub layout: vk::DescriptorSetLayout,
}

impl DescriptorSetLayout {
    pub fn new(device: &Device) -> Result<Self, vk::Result> {
        let binding = vk::DescriptorSetLayoutBinding::default()
            .binding(0)
            .descriptor_count(1)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .stage_flags(vk::ShaderStageFlags::FRAGMENT);

        let info =
            vk::DescriptorSetLayoutCreateInfo::default().bindings(std::slice::from_ref(&binding));

        let layout = unsafe { device.create_descriptor_set_layout(&info, None)? };

        println!("Descriptor Set Layout created.");

        Ok(Self { layout })
    }
}
