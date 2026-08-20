#![cfg(feature = "virglrenderer-backend")]

use std::sync::{Arc, Mutex};

use virglrenderer::{
    FenceHandler, ResourceCreateBlob as VirglResourceCreateBlob, VirglRenderer, VirglRendererFlags,
};

use crate::virtio_gpu::protocol::commands::CAPSET_VENUS;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompletedFence {
    pub fence_id: u64,
    pub ctx_id: u32,
    pub ring_idx: u8,
}

#[derive(Default)]
struct FenceSink {
    completed: Mutex<Vec<CompletedFence>>,
}

impl FenceSink {
    fn drain(&self) -> Vec<CompletedFence> {
        let mut completed = self.completed.lock().expect("fence sink poisoned");
        std::mem::take(&mut *completed)
    }
}

impl FenceHandler for FenceSink {
    fn call(&self, fence_id: u64, ctx_id: u32, ring_idx: u8) {
        self.completed
            .lock()
            .expect("fence sink poisoned")
            .push(CompletedFence {
                fence_id,
                ctx_id,
                ring_idx,
            });
    }
}

struct FenceSinkProxy(Arc<FenceSink>);

impl FenceHandler for FenceSinkProxy {
    fn call(&self, fence_id: u64, ctx_id: u32, ring_idx: u8) {
        self.0.call(fence_id, ctx_id, ring_idx)
    }
}

pub struct VirglVenusBackend {
    renderer: Arc<VirglRenderer>,
    fence_sink: Arc<FenceSink>,
}

impl VirglVenusBackend {
    pub fn new() -> Result<Self, virglrenderer::VirglError> {
        let fence_sink = Arc::new(FenceSink::default());
        let flags = VirglRendererFlags::new()
            .use_virgl(true)
            .use_venus(true)
            .use_surfaceless(true)
            .use_external_blob(true)
            .use_async_fence_cb(true)
            .use_thread_sync(true);

        let renderer = VirglRenderer::init(
            flags,
            Box::new(FenceSinkProxy(fence_sink.clone())),
            None,
            None,
        )?;

        Ok(Self {
            renderer: Arc::new(renderer),
            fence_sink,
        })
    }

    pub fn get_capset_info(&self) -> (u32, u32) {
        self.renderer.get_capset_info(CAPSET_VENUS)
    }

    pub fn get_capset(&self, version: u32) -> Vec<u8> {
        self.renderer.get_capset(CAPSET_VENUS, version)
    }

    pub fn create_context(
        &self,
        ctx_id: u32,
        context_init: u32,
        name: Option<&str>,
    ) -> Result<(), virglrenderer::VirglError> {
        self.renderer.create_context(ctx_id, context_init, name)
    }

    pub fn destroy_context(&self, ctx_id: u32) {
        self.renderer.destroy_context(ctx_id)
    }

    pub fn attach_resource(&self, ctx_id: u32, resource_id: u32) {
        self.renderer.ctx_attach_resource(ctx_id, resource_id)
    }

    pub fn detach_resource(&self, ctx_id: u32, resource_id: u32) {
        self.renderer.ctx_detach_resource(ctx_id, resource_id)
    }

    pub fn create_blob(
        &self,
        ctx_id: u32,
        width: u32,
        height: u32,
        resource_id: u32,
        blob_mem: u32,
        blob_flags: u32,
        blob_id: u64,
        size: u64,
    ) -> Result<u32, virglrenderer::VirglError> {
        let blob = VirglResourceCreateBlob {
            blob_mem,
            blob_flags,
            blob_id,
            size,
        };
        let resource = self
            .renderer
            .create_blob(ctx_id, width, height, resource_id, blob, None)?;
        Ok(resource.resource_id)
    }

    pub fn submit(
        &self,
        ctx_id: u32,
        flags: u32,
        ring_idx: u8,
        fence_id: u64,
        commands: &mut [u8],
        in_fences: &[u64],
    ) -> Result<(), virglrenderer::VirglError> {
        self.renderer.submit_cmd(ctx_id, commands, in_fences)?;
        self.renderer
            .context_create_fence(ctx_id, flags, ring_idx as u32, fence_id)?;
        Ok(())
    }

    pub fn poll(&self) -> Vec<CompletedFence> {
        self.renderer.event_poll();
        self.fence_sink.drain()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn backend_is_feature_gated() {
        assert!(cfg!(feature = "virglrenderer-backend"));
    }
}
