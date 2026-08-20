use ash::{Device, vk};

use super::{DescriptorSet, Image, Sampler};

pub fn update(device: &Device, descriptor_set: &DescriptorSet, sampler: &Sampler, image: &Image) {
    let image_info = vk::DescriptorImageInfo::default()
        .sampler(sampler.sampler)
        .image_view(image.view)
        .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);

    let write = vk::WriteDescriptorSet::default()
        .dst_set(descriptor_set.set)
        .dst_binding(0)
        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
        .image_info(std::slice::from_ref(&image_info));

    unsafe {
        device.update_descriptor_sets(std::slice::from_ref(&write), &[]);
    }

    println!("Descriptor Set updated.");
}
