#![cfg(feature = "virglrenderer-backend")]

use crate::virtio_gpu::protocol::commands::*;
use crate::virtio_gpu::protocol::header::CtrlHeader;
use crate::virtio_gpu::protocol::requests::blob::{MemEntry, ResourceCreateBlob, ResourceMapBlob, ResourceUnmapBlob};
use crate::virtio_gpu::protocol::requests::standard::{ResourceAssignUuid, ResourceDetachBacking, ResourceUnref};
use crate::virtio_gpu::protocol::requests::submit::Submit3D;
use crate::virtio_gpu::protocol::responses::{RespOkNoData, RespResourceUuid};
use crate::virtio_gpu::transport::memory::GuestMemory;

use super::virgl::{CompletedFence, VirglVenusBackend};
use super::{VenusDispatchError, VenusResponse, VenusState, VenusStateError};

#[derive(Debug)]
pub enum VenusRuntimeError {
    Backend(virglrenderer::VirglError),
    Dispatch(VenusDispatchError),
    State(VenusStateError),
}

impl From<virglrenderer::VirglError> for VenusRuntimeError {
    fn from(value: virglrenderer::VirglError) -> Self {
        Self::Backend(value)
    }
}
impl From<VenusDispatchError> for VenusRuntimeError {
    fn from(value: VenusDispatchError) -> Self {
        Self::Dispatch(value)
    }
}
impl From<VenusStateError> for VenusRuntimeError {
    fn from(value: VenusStateError) -> Self {
        Self::State(value)
    }
}

#[derive(Clone, Copy)]
struct TransferRequest {
    resource_id: u32,
    ctx_id: u32,
    x: u32,
    y: u32,
    z: u32,
    w: u32,
    h: u32,
    d: u32,
    level: u32,
    stride: u32,
    layer_stride: u32,
    offset: u64,
}

impl TransferRequest {
    fn decode(request: &[u8], expected_type: u32) -> Result<Self, VenusDispatchError> {
        if request.len() < 72 {
            return Err(VenusDispatchError::InvalidRequest);
        }
        let header = CtrlHeader::decode_le(&request[..24]).ok_or(VenusDispatchError::InvalidRequest)?;
        if header.typ != expected_type {
            return Err(VenusDispatchError::InvalidRequest);
        }
        let u32_at = |start: usize| -> Result<u32, VenusDispatchError> {
            let end = start.checked_add(4).ok_or(VenusDispatchError::InvalidRequest)?;
            Ok(u32::from_le_bytes(request[start..end].try_into().map_err(|_| VenusDispatchError::InvalidRequest)?))
        };
        let u64_at = |start: usize| -> Result<u64, VenusDispatchError> {
            let end = start.checked_add(8).ok_or(VenusDispatchError::InvalidRequest)?;
            Ok(u64::from_le_bytes(request[start..end].try_into().map_err(|_| VenusDispatchError::InvalidRequest)?))
        };
        Ok(Self {
            resource_id: u32_at(56)?,
            ctx_id: header.ctx_id,
            x: u32_at(24)?,
            y: u32_at(28)?,
            z: u32_at(32)?,
            w: u32_at(36)?,
            h: u32_at(40)?,
            d: u32_at(44)?,
            level: u32_at(60)?,
            stride: u32_at(64)?,
            layer_stride: u32_at(68)?,
            offset: u64_at(48)?,
        })
    }

    fn as_virgl(self) -> virglrenderer::Transfer3D {
        virglrenderer::Transfer3D {
            x: self.x,
            y: self.y,
            z: self.z,
            w: self.w,
            h: self.h,
            d: self.d,
            level: self.level,
            stride: self.stride,
            layer_stride: self.layer_stride,
            offset: self.offset,
        }
    }
}

pub struct VenusRuntime {
    pub state: VenusState,
    pub backend: VirglVenusBackend,
    guest_memory: GuestMemory,
}

impl VenusRuntime {
    pub fn new(guest_memory: GuestMemory) -> Result<Self, VenusRuntimeError> {
        let backend = VirglVenusBackend::new()?;
        let (version, size) = backend.get_capset_info();
        let mut state = VenusState::new();
        state.capset_version = version;
        state.capset_size = size;
        Ok(Self { state, backend, guest_memory })
    }

    pub fn detach_backing(&mut self, resource_id: u32) {
        self.backend.detach_backing(resource_id);
    }

    fn ok(header: CtrlHeader) -> VenusResponse {
        VenusResponse {
            bytes: RespOkNoData { header: CtrlHeader { typ: RESP_OK_NODATA, ..header } }.encode_le(),
            fence: (header.flags & FLAG_FENCE != 0).then_some(header.fence_id),
        }
    }

    fn capset_info(&self, header: CtrlHeader) -> VenusResponse {
        let (version, size) = self.backend.get_capset_info();
        let mut bytes = Vec::with_capacity(40);
        bytes.extend_from_slice(&CtrlHeader { typ: RESP_OK_CAPSET_INFO, ..header }.encode_le());
        bytes.extend_from_slice(&CAPSET_VENUS.to_le_bytes());
        bytes.extend_from_slice(&version.to_le_bytes());
        bytes.extend_from_slice(&size.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        VenusResponse { bytes, fence: (header.flags & FLAG_FENCE != 0).then_some(header.fence_id) }
    }

    fn capset(&self, header: CtrlHeader, version: u32) -> Result<VenusResponse, VenusRuntimeError> {
        let (max_version, size) = self.backend.get_capset_info();
        if version == 0 || version > max_version || size == 0 {
            return Err(VenusStateError::UnsupportedCapability.into());
        }
        let data = self.backend.get_capset(version);
        let mut bytes = Vec::with_capacity(24 + data.len());
        bytes.extend_from_slice(&CtrlHeader { typ: RESP_OK_CAPSET, ..header }.encode_le());
        bytes.extend_from_slice(&data);
        Ok(VenusResponse { bytes, fence: (header.flags & FLAG_FENCE != 0).then_some(header.fence_id) })
    }

    fn read_u32(request: &[u8], start: usize) -> Result<u32, VenusDispatchError> {
        let end = start.checked_add(4).ok_or(VenusDispatchError::InvalidRequest)?;
        Ok(u32::from_le_bytes(request[start..end].try_into().map_err(|_| VenusDispatchError::InvalidRequest)?))
    }

    fn read_u64(request: &[u8], start: usize) -> Result<u64, VenusDispatchError> {
        let end = start.checked_add(8).ok_or(VenusDispatchError::InvalidRequest)?;
        Ok(u64::from_le_bytes(request[start..end].try_into().map_err(|_| VenusDispatchError::InvalidRequest)?))
    }

    fn decode_entries(request: &[u8], start: usize, count: u32) -> Result<Vec<MemEntry>, VenusDispatchError> {
        let count = usize::try_from(count).map_err(|_| VenusDispatchError::InvalidRequest)?;
        let bytes = count.checked_mul(MemEntry::SIZE).and_then(|n| start.checked_add(n)).ok_or(VenusDispatchError::InvalidRequest)?;
        if request.len() < bytes { return Err(VenusDispatchError::InvalidRequest); }
        let mut entries = Vec::with_capacity(count);
        for chunk in request[start..bytes].chunks_exact(MemEntry::SIZE) {
            entries.push(MemEntry::decode_le(chunk).ok_or(VenusDispatchError::InvalidRequest)?);
        }
        Ok(entries)
    }

    pub fn dispatch(&mut self, request: &[u8]) -> Result<VenusResponse, VenusRuntimeError> {
        let header = CtrlHeader::decode_le(request).ok_or(VenusDispatchError::InvalidRequest)?;
        match header.typ {
            CMD_GET_CAPSET_INFO => {
                let index = Self::read_u32(request, 24)?;
                if index != 0 { return Err(VenusStateError::UnsupportedCapability.into()); }
                Ok(self.capset_info(header))
            }
            CMD_GET_CAPSET => {
                let capset_id = Self::read_u32(request, 24)?;
                let version = Self::read_u32(request, 28)?;
                if capset_id != CAPSET_VENUS { return Err(VenusStateError::UnsupportedCapability.into()); }
                self.capset(header, version)
            }
            CMD_RESOURCE_ATTACH_BACKING => {
                let resource_id = Self::read_u32(request, 24)?;
                let count = Self::read_u32(request, 28)?;
                if !self.state.resources.contains_key(&resource_id) { return Err(VenusStateError::InvalidResource.into()); }
                let entries = Self::decode_entries(request, 32, count)?;
                self.backend.attach_backing(resource_id, &self.guest_memory, &entries)?;
                Ok(Self::ok(header))
            }
            CMD_RESOURCE_DETACH_BACKING => {
                let req = ResourceDetachBacking::decode_le(request).ok_or(VenusDispatchError::InvalidRequest)?;
                if !self.state.resources.contains_key(&req.resource_id) { return Err(VenusStateError::InvalidResource.into()); }
                self.detach_backing(req.resource_id);
                Ok(Self::ok(header))
            }
            CMD_RESOURCE_UNREF => {
                let req = ResourceUnref::decode_le(request).ok_or(VenusDispatchError::InvalidRequest)?;
                let resource = self.state.resources.get(&req.resource_id).ok_or(VenusStateError::InvalidResource)?;
                if !resource.attached_contexts.is_empty() { return Err(VenusStateError::ResourceInUse.into()); }
                self.backend.unref_resource(req.resource_id);
                self.state.unref_resource(req.resource_id)?;
                Ok(Self::ok(header))
            }
            CMD_RESOURCE_MAP_BLOB => {
                let req = ResourceMapBlob::decode_le(request).ok_or(VenusDispatchError::InvalidRequest)?;
                let (_size, map_info) = self.backend.map_resource(req.resource_id)?;
                self.state.resources.get_mut(&req.resource_id).ok_or(VenusStateError::InvalidResource)?.map(req.offset)?;
                let mut bytes = Vec::with_capacity(32);
                bytes.extend_from_slice(&CtrlHeader { typ: RESP_OK_MAP_INFO, ..header }.encode_le());
                bytes.extend_from_slice(&map_info.to_le_bytes());
                bytes.extend_from_slice(&0u32.to_le_bytes());
                Ok(VenusResponse { bytes, fence: (header.flags & FLAG_FENCE != 0).then_some(header.fence_id) })
            }
            CMD_RESOURCE_UNMAP_BLOB => {
                let req = ResourceUnmapBlob::decode_le(request).ok_or(VenusDispatchError::InvalidRequest)?;
                let resource = self.state.resources.get(&req.resource_id).ok_or(VenusStateError::InvalidResource)?;
                if resource.mapped_offset.is_none() { return Err(VenusStateError::NotMapped.into()); }
                self.backend.unmap_resource(req.resource_id)?;
                self.state.resources.get_mut(&req.resource_id).expect("resource checked above").unmap()?;
                Ok(Self::ok(header))
            }
            _ => Err(VenusDispatchError::UnsupportedCommand.into()),
        }
    }

    pub fn poll_fences(&self) -> Vec<CompletedFence> {
        self.backend.poll()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn runtime_error_is_send_sync_safe() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<super::VenusRuntimeError>();
    }
}
