use std::collections::HashMap;

use crate::virtio_gpu::resource::ResourceId;

use super::{DeviceMemory, Image};

pub struct GpuTexture {
    pub image: Image,
    pub memory: DeviceMemory,
}

#[derive(Default)]
pub struct ResourceManager {
    textures: HashMap<ResourceId, GpuTexture>,
}

impl ResourceManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, id: ResourceId, texture: GpuTexture) -> Option<GpuTexture> {
        self.textures.insert(id, texture)
    }

    pub fn get(&self, id: ResourceId) -> Option<&GpuTexture> {
        self.textures.get(&id)
    }

    pub fn get_mut(&mut self, id: ResourceId) -> Option<&mut GpuTexture> {
        self.textures.get_mut(&id)
    }

    pub fn remove(&mut self, id: ResourceId) -> Option<GpuTexture> {
        self.textures.remove(&id)
    }

    pub fn len(&self) -> usize {
        self.textures.len()
    }

    pub fn is_empty(&self) -> bool {
        self.textures.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manager_starts_empty() {
        let manager = ResourceManager::new();
        assert!(manager.is_empty());
        assert_eq!(manager.len(), 0);
    }
}
