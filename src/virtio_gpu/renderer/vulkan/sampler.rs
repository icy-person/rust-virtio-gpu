use ash::{Device, vk};

pub struct Sampler {
    pub sampler: vk::Sampler,
}

impl Sampler {
    pub fn new(device: &Device) -> Result<Self, vk::Result> {
        let info = vk::SamplerCreateInfo::default()
            .mag_filter(vk::Filter::LINEAR)
            .min_filter(vk::Filter::LINEAR)
            .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
            .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
            .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE)
            .anisotropy_enable(false)
            .border_color(vk::BorderColor::FLOAT_OPAQUE_BLACK)
            .unnormalized_coordinates(false)
            .compare_enable(false)
            .mipmap_mode(vk::SamplerMipmapMode::LINEAR);

        let sampler = unsafe { device.create_sampler(&info, None)? };

        println!("Sampler created.");

        Ok(Self { sampler })
    }
}
