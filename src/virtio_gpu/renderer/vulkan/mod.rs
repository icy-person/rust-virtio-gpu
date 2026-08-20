pub mod barrier;
pub mod buffer;
pub mod command_buffer;
pub mod command_pool;
pub mod copy;
pub mod descriptor;
pub mod descriptor_pool;
pub mod descriptor_set;
pub mod device;
pub mod framebuffer;
pub mod graphics_pipeline;
pub mod image;
pub mod image_view;
pub mod index_buffer;
pub mod instance;
pub mod map_memory;
pub mod memory;
pub mod physical_device;
pub mod queue;
pub mod render;
pub mod render_pass;
pub mod resource_manager;
pub mod sampler;
pub mod shader;
pub mod submit;
pub mod swapchain;
pub mod update_descriptor;
pub mod upload;
pub mod vertex;
pub mod vertex_buffer;

use super::framebuffer::FrameBuffer;
use super::renderer::Renderer;
use crate::virtio_gpu::device::DeviceError;
use crate::virtio_gpu::resource::Resource;
use ash::vk;
pub use buffer::Buffer;
pub use command_buffer::CommandBuffer;
pub use command_pool::CommandPool;
pub use descriptor::DescriptorSetLayout;
pub use descriptor_pool::DescriptorPool;
pub use descriptor_set::DescriptorSet;
pub use device::LogicalDevice;
pub use framebuffer::VulkanFramebuffer;
pub use graphics_pipeline::GraphicsPipeline;
pub use image::Image;
pub use image_view::ImageView;
pub use index_buffer::IndexBuffer;
pub use instance::VulkanInstance;
pub use memory::DeviceMemory;
pub use physical_device::PhysicalDevice;
pub use queue::QueueFamily;
pub use render_pass::RenderPass;
pub use resource_manager::*;
pub use sampler::Sampler;
pub use shader::ShaderModule;
pub use swapchain::Swapchain;
pub use upload::upload_resource;
pub use vertex::{FULLSCREEN_QUAD, INDICES, Vertex};
pub use vertex_buffer::VertexBuffer;

pub struct VulkanRenderer {
    framebuffer: FrameBuffer,

    pub instance: VulkanInstance,
    pub physical_device: PhysicalDevice,

    pub graphics_queue_family: u32,

    pub logical_device: LogicalDevice,

    pub command_pool: CommandPool,
    pub command_buffer: CommandBuffer,

    pub staging_buffer: Buffer,
    pub staging_memory: DeviceMemory,

    pub image: Image,
    pub image_memory: DeviceMemory,

    pub swapchain: Swapchain,
    pub image_view: ImageView,
    pub render_pass: RenderPass,
    pub framebuffer_vk: VulkanFramebuffer,

    pub vertex_shader: ShaderModule,
    pub fragment_shader: ShaderModule,

    pub descriptor_layout: DescriptorSetLayout,

    pub descriptor_pool: DescriptorPool,
    pub descriptor_set: DescriptorSet,

    pub sampler: Sampler,

    pub vertex_buffer: VertexBuffer,
    pub index_buffer: IndexBuffer,

    pub pipeline: GraphicsPipeline,
}

impl VulkanRenderer {
    pub fn new(width: u32, height: u32) -> Self {
        let instance = VulkanInstance::new().expect("Failed to create Vulkan Instance");

        let physical_device =
            PhysicalDevice::pick_best(&instance.instance).expect("Failed to pick GPU");

        let queues = QueueFamily::enumerate(&instance.instance, physical_device.physical_device);

        let graphics_queue_family =
            QueueFamily::graphics(&queues).expect("Graphics Queue not found");

        let logical_device = LogicalDevice::new(
            &instance.instance,
            physical_device.physical_device,
            graphics_queue_family,
        )
        .expect("Failed to create Logical Device");
        let command_pool = CommandPool::new(&logical_device.device, graphics_queue_family)
            .expect("Failed to create Command Pool");
        let command_buffer = CommandBuffer::new(&logical_device.device, command_pool.pool)
            .expect("Failed to allocate Command Buffer");

        let staging_buffer = Buffer::new(
            &logical_device.device,
            (width * height * 4) as u64,
            ash::vk::BufferUsageFlags::TRANSFER_SRC | ash::vk::BufferUsageFlags::TRANSFER_DST,
        )
        .expect("Failed to create staging buffer");

        let staging_memory = DeviceMemory::allocate_buffer(
            &instance.instance,
            physical_device.physical_device,
            &logical_device.device,
            staging_buffer.buffer,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        )
        .expect("Failed to allocate staging memory");

        unsafe {
            logical_device
                .device
                .bind_buffer_memory(staging_buffer.buffer, staging_memory.memory, 0)
                .expect("Failed to bind staging buffer");
        }

        let image =
            Image::new(&logical_device.device, width, height).expect("Failed to create image");

        let image_memory = DeviceMemory::allocate_image(
            &instance.instance,
            physical_device.physical_device,
            &logical_device.device,
            image.image,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
        )
        .expect("Failed to allocate image memory");

        unsafe {
            logical_device
                .device
                .bind_image_memory(image.image, image_memory.memory, 0)
                .expect("Failed to bind image");
        }

        let swapchain = Swapchain::new(&instance.instance, &logical_device.device);

        let image_view = ImageView::new(&logical_device.device, image.image)
            .expect("Failed to create Image View");

        let render_pass =
            RenderPass::new(&logical_device.device).expect("Failed to create Render Pass");

        let framebuffer_vk = VulkanFramebuffer::new(
            &logical_device.device,
            render_pass.render_pass,
            image_view.view,
            width,
            height,
        )
        .expect("Failed to create Framebuffer");

        let vertex_shader =
            ShaderModule::load(&logical_device.device, "assets/shaders/triangle.vert.spv")
                .expect("Vertex shader");

        let fragment_shader =
            ShaderModule::load(&logical_device.device, "assets/shaders/triangle.frag.spv")
                .expect("Fragment shader");

        let descriptor_layout =
            DescriptorSetLayout::new(&logical_device.device).expect("Descriptor Layout");

        let descriptor_pool = DescriptorPool::new(&logical_device.device).expect("Descriptor Pool");

        let descriptor_set =
            DescriptorSet::new(&logical_device.device, &descriptor_layout, &descriptor_pool)
                .expect("Descriptor Set");

        // =========================
        // Vertex Buffer
        // =========================

        let vertex_size = (std::mem::size_of::<Vertex>() * FULLSCREEN_QUAD.len()) as u64;

        let vertex_buffer_raw = Buffer::new(
            &logical_device.device,
            vertex_size,
            ash::vk::BufferUsageFlags::VERTEX_BUFFER,
        )
        .expect("Vertex Buffer");

        let vertex_memory = DeviceMemory::allocate_buffer(
            &instance.instance,
            physical_device.physical_device,
            &logical_device.device,
            vertex_buffer_raw.buffer,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        )
        .expect("Vertex Memory");

        unsafe {
            logical_device
                .device
                .bind_buffer_memory(vertex_buffer_raw.buffer, vertex_memory.memory, 0)
                .expect("Bind Vertex Buffer");
        }

        let vertex_buffer = VertexBuffer::new(
            &instance.instance,
            physical_device.physical_device,
            &logical_device.device,
            &FULLSCREEN_QUAD,
        )
        .expect("Vertex Buffer");

        let index_buffer = IndexBuffer::new(
            &instance.instance,
            physical_device.physical_device,
            &logical_device.device,
            &INDICES,
        )
        .expect("Index Buffer");
        let sampler = Sampler::new(&logical_device.device).expect("Sampler");

        update_descriptor::update(&logical_device.device, &descriptor_set, &sampler, &image);

        let pipeline = GraphicsPipeline::new(
            &logical_device.device,
            &render_pass,
            &descriptor_layout,
            &vertex_shader,
            &fragment_shader,
        )
        .expect("Pipeline");
        Self {
            framebuffer: FrameBuffer::new(width, height),

            instance,
            physical_device,

            graphics_queue_family,

            logical_device,

            command_pool,
            command_buffer,

            staging_buffer,
            staging_memory,

            image,
            image_memory,

            swapchain,
            image_view,
            render_pass,

            framebuffer_vk,

            vertex_shader,
            fragment_shader,

            descriptor_layout,
            descriptor_pool,
            descriptor_set,

            sampler,

            vertex_buffer,
            index_buffer,

            pipeline,
        }
    }

    pub fn transfer_resource(&mut self, resource: &mut Resource) -> Result<(), DeviceError> {
        self.command_buffer
            .reset(&self.logical_device.device, self.command_pool.pool)
            .map_err(|_| DeviceError::InvalidParameter)?;

        crate::virtio_gpu::renderer::vulkan::map_memory::write(
            &self.logical_device.device,
            self.staging_memory.memory,
            resource.pixels(),
        )
        .map_err(|_| DeviceError::InvalidParameter)?;

        self.command_buffer
            .begin(&self.logical_device.device)
            .map_err(|_| DeviceError::InvalidParameter)?;

        barrier::transition_image(
            &self.logical_device.device,
            self.command_buffer.buffer,
            self.image.image,
        );

        copy::buffer_to_image(
            &self.logical_device.device,
            self.command_buffer.buffer,
            self.staging_buffer.buffer,
            self.image.image,
            resource.width,
            resource.height,
        );

        render::record(
            &self.logical_device.device,
            self.command_buffer.buffer,
            &self.render_pass,
            &self.framebuffer_vk,
            &self.pipeline,
            &self.vertex_buffer,
            &self.index_buffer,
            &self.descriptor_set,
            resource.width,
            resource.height,
        )
        .map_err(|_| DeviceError::InvalidParameter)?;

        self.command_buffer
            .end(&self.logical_device.device)
            .map_err(|_| DeviceError::InvalidParameter)?;
        submit::submit(
            &self.logical_device.device,
            self.logical_device.graphics_queue,
            self.command_buffer.buffer,
        )
        .map_err(|_| DeviceError::InvalidParameter)?;

        println!("Uploaded {} bytes to GPU", resource.pixels().len());

        Ok(())
    }
}

impl Renderer for VulkanRenderer {
    fn upload(&mut self, data: &[u8]) {
        self.framebuffer.update(data);
    }

    fn framebuffer(&self) -> &FrameBuffer {
        &self.framebuffer
    }

    fn framebuffer_mut(&mut self) -> &mut FrameBuffer {
        &mut self.framebuffer
    }

    fn transfer_resource(&mut self, resource: &mut Resource) -> Result<(), DeviceError> {
        VulkanRenderer::transfer_resource(self, resource)
    }

    fn flush_resource(&mut self, resource: &mut Resource) -> Result<(), DeviceError> {
        self.transfer_resource(resource)?;

        Ok(())
    }
}
