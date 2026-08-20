#![cfg(feature = "virglrenderer-backend")]

use std::collections::HashMap;
use std::io::{IoSlice, IoSliceMut};
use std::sync::{Arc, Mutex};

use virglrenderer::{
    FenceHandler, Iovec, ResourceCreate3D, ResourceCreateBlob as VirglResourceCreateBlob,
    Transfer3D, VirglRenderer, VirglRendererFlags,
};

use crate::virtio_gpu::protocol::commands::{CAPSET_VENUS, FLAG_FENCE};
use crate::virtio_gpu::protocol::requests::blob::MemEntry;
use crate::virtio_gpu::transport::memory::{GuestAddress, GuestMemory, GuestMemoryError};

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
    backings: Mutex<HashMap<u32, Vec<Iovec>>>,
    map_info: Mutex<HashMap<u32, u32>>,
}

impl VirglVenusBackend {
    pub fn new() -> Result<Self, virglrenderer::VirglError> {
        let fence_sink = Arc::new(FenceSink::default());
        let flags = VirglRendererFlags::new()
            .use_egl(true)
            .use_virgl(true)
            .use_venus(true)
            .use_surfaceless(true)
            .use_external_blob(true)
            .use_async_fence_cb(true)
            .use_thread_sync(true);

        let renderer =
            VirglRenderer::init(flags, Box::new(FenceSinkProxy(fence_sink.clone())), None)?;

        Ok(Self {
            renderer: Arc::new(renderer),
            fence_sink,
            backings: Mutex::new(HashMap::new()),
            map_info: Mutex::new(HashMap::new()),
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

    fn make_iovecs(
        &self,
        memory: &GuestMemory,
        entries: &[MemEntry],
    ) -> Result<Vec<Iovec>, GuestMemoryError> {
        entries
            .iter()
            .map(|entry| {
                let ptr = memory.as_mut_ptr(
                    GuestAddress::new(entry.addr),
                    usize::try_from(entry.length).map_err(|_| GuestMemoryError::AddressOverflow)?,
                )?;
                Ok(Iovec {
                    base: ptr.cast(),
                    len: entry.length as usize,
                })
            })
            .collect()
    }

    fn backend_io_error() -> virglrenderer::VirglError {
        virglrenderer::VirglError::IoError(std::io::Error::from_raw_os_error(14))
    }

    pub fn create_3d(
        &self,
        resource_id: u32,
        target: u32,
        format: u32,
        bind: u32,
        width: u32,
        height: u32,
        depth: u32,
        array_size: u32,
        last_level: u32,
        nr_samples: u32,
        flags: u32,
    ) -> Result<(), virglrenderer::VirglError> {
        self.renderer.create_3d(
            resource_id,
            ResourceCreate3D {
                target,
                format,
                bind,
                width,
                height,
                depth,
                array_size,
                last_level,
                nr_samples,
                flags,
            },
        )?;
        self.map_info
            .lock()
            .expect("virgl map-info map poisoned")
            .insert(resource_id, 0);
        Ok(())
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
        memory: &GuestMemory,
        entries: &[MemEntry],
    ) -> Result<u32, virglrenderer::VirglError> {
        let blob = VirglResourceCreateBlob {
            blob_mem,
            blob_flags,
            blob_id,
            size,
        };

        let iovecs = if entries.is_empty() {
            Vec::new()
        } else {
            self.make_iovecs(memory, entries)
                .map_err(|_| Self::backend_io_error())?
        };

        let resource = self.renderer.create_blob(
            ctx_id,
            width,
            height,
            resource_id,
            blob,
            (!iovecs.is_empty()).then_some(iovecs.as_slice()),
        )?;

        if !iovecs.is_empty() {
            self.backings
                .lock()
                .expect("virgl backing map poisoned")
                .insert(resource.resource_id, iovecs);
        }

        self.map_info
            .lock()
            .expect("virgl map-info map poisoned")
            .insert(resource.resource_id, resource.map_info.unwrap_or(0));

        Ok(resource.resource_id)
    }

    pub fn attach_backing(
        &self,
        resource_id: u32,
        memory: &GuestMemory,
        entries: &[MemEntry],
    ) -> Result<(), virglrenderer::VirglError> {
        let mut iovecs = self
            .make_iovecs(memory, entries)
            .map_err(|_| Self::backend_io_error())?;
        self.renderer.attach_backing(resource_id, &mut iovecs)?;
        self.backings
            .lock()
            .expect("virgl backing map poisoned")
            .insert(resource_id, iovecs);
        Ok(())
    }

    pub fn detach_backing(&self, resource_id: u32) {
        self.renderer.detach_backing(resource_id);
        self.backings
            .lock()
            .expect("virgl backing map poisoned")
            .remove(&resource_id);
    }

    pub fn unref_resource(&self, resource_id: u32) {
        self.renderer.unref_resource(resource_id);
        self.backings
            .lock()
            .expect("virgl backing map poisoned")
            .remove(&resource_id);
        self.map_info
            .lock()
            .expect("virgl map-info map poisoned")
            .remove(&resource_id);
    }

    pub fn map_resource(
        &self,
        resource_id: u32,
    ) -> Result<(u64, u32), virglrenderer::VirglError> {
        let (_ptr, size) = self.renderer.map(resource_id)?;
        let map_info = *self
            .map_info
            .lock()
            .expect("virgl map-info map poisoned")
            .get(&resource_id)
            .unwrap_or(&0);
        Ok((size, map_info))
    }

    pub fn unmap_resource(&self, resource_id: u32) -> Result<(), virglrenderer::VirglError> {
        self.renderer.unmap(resource_id)
    }

    pub fn transfer_write(
        &self,
        resource_id: u32,
        ctx_id: u32,
        transfer: Transfer3D,
    ) -> Result<(), virglrenderer::VirglError> {
        self.renderer.transfer_write(resource_id, ctx_id, transfer, None)
    }

    pub fn transfer_read_to_guest(
        &self,
        resource_id: u32,
        ctx_id: u32,
        transfer: Transfer3D,
        memory: &GuestMemory,
        entries: &[MemEntry],
    ) -> Result<(), virglrenderer::VirglError> {
        let total = entries.iter().try_fold(0usize, |acc, e| {
            acc.checked_add(e.length as usize)
                .ok_or_else(Self::backend_io_error)
        })?;
        let mut staging = vec![0u8; total];
        {
            let io = IoSliceMut::new(&mut staging);
            self.renderer
                .transfer_read(resource_id, ctx_id, transfer, Some(io))?;
        }
        let mut copied = 0usize;
        for entry in entries {
            let len = entry.length as usize;
            memory
                .write(GuestAddress::new(entry.addr), &staging[copied..copied + len])
                .map_err(|_| Self::backend_io_error())?;
            copied += len;
        }
        Ok(())
    }

    #[allow(dead_code)]
    fn transfer_write_from_guest_staging(
        &self,
        resource_id: u32,
        ctx_id: u32,
        transfer: Transfer3D,
        data: &[u8],
    ) -> Result<(), virglrenderer::VirglError> {
        let io = IoSlice::new(data);
        self.renderer
            .transfer_write(resource_id, ctx_id, transfer, Some(&io))
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

        if flags & FLAG_FENCE != 0 {
            self.renderer
                .context_create_fence(ctx_id, flags, ring_idx as u32, fence_id)?;
        }
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
