use std::collections::HashMap;

use crate::virtio_gpu::resource::ResourceId;

use super::{DeviceMemory, Image};

pub struct GpuTexture {
    pub image: Image,

    pub memory: DeviceMemory,
}

pub struct ResourceManager {
    #[allow(dead_code)]
    textures: HashMap<ResourceId, GpuTexture>,
}

impl ResourceManager {
    pub fn new() -> Self {
        Self {
            textures: HashMap::new(),
        }
    }
}
