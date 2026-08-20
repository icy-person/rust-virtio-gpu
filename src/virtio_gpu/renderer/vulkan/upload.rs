use ash::Device;

use crate::virtio_gpu::resource::Resource;

pub fn upload_resource(_device: &Device, resource: &Resource) -> Result<(), ash::vk::Result> {
    println!(
        "Uploading Resource {} ({} bytes)",
        resource.id,
        resource.data.len()
    );

    Ok(())
}
