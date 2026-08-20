use ash::{Device, vk};

use super::{
    DescriptorSet, GraphicsPipeline, IndexBuffer, RenderPass, VertexBuffer, VulkanFramebuffer,
};

pub fn record(
    device: &Device,
    cmd: vk::CommandBuffer,

    render_pass: &RenderPass,
    framebuffer: &VulkanFramebuffer,

    pipeline: &GraphicsPipeline,

    vertex_buffer: &VertexBuffer,
    index_buffer: &IndexBuffer,

    descriptor_set: &DescriptorSet,

    width: u32,
    height: u32,
) -> Result<(), vk::Result> {
    let clear = vk::ClearValue {
        color: vk::ClearColorValue {
            float32: [0.0, 0.0, 0.0, 1.0],
        },
    };

    let begin = vk::RenderPassBeginInfo::default()
        .render_pass(render_pass.render_pass)
        .framebuffer(framebuffer.framebuffer)
        .render_area(vk::Rect2D {
            offset: vk::Offset2D { x: 0, y: 0 },
            extent: vk::Extent2D { width, height },
        })
        .clear_values(std::slice::from_ref(&clear));

    unsafe {
        device.cmd_begin_render_pass(cmd, &begin, vk::SubpassContents::INLINE);

        pipeline.bind(device, cmd);

        let viewport = vk::Viewport {
            x: 0.0,
            y: 0.0,
            width: width as f32,
            height: height as f32,
            min_depth: 0.0,
            max_depth: 1.0,
        };

        device.cmd_set_viewport(cmd, 0, std::slice::from_ref(&viewport));

        let scissor = vk::Rect2D {
            offset: vk::Offset2D { x: 0, y: 0 },
            extent: vk::Extent2D { width, height },
        };

        device.cmd_set_scissor(cmd, 0, std::slice::from_ref(&scissor));

        vertex_buffer.bind(device, cmd);

        index_buffer.bind(device, cmd);

        device.cmd_bind_descriptor_sets(
            cmd,
            vk::PipelineBindPoint::GRAPHICS,
            pipeline.layout,
            0,
            std::slice::from_ref(&descriptor_set.set),
            &[],
        );
        device.cmd_draw_indexed(cmd, index_buffer.index_count, 1, 0, 0, 0);

        device.cmd_end_render_pass(cmd);
    }

    println!(
        "Recorded indexed draw ({} indices).",
        index_buffer.index_count,
    );

    Ok(())
}
