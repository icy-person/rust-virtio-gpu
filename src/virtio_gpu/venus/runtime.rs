#![cfg(feature = "virglrenderer-backend")]

use crate::virtio_gpu::protocol::commands::*;
use crate::virtio_gpu::protocol::header::CtrlHeader;
use crate::virtio_gpu::protocol::requests::blob::{
    MemEntry, ResourceCreateBlob, ResourceMapBlob, ResourceUnmapBlob,
};
use crate::virtio_gpu::protocol::requests::capset::{GetCapset, GetCapsetInfo};
use crate::virtio_gpu::protocol::requests::context::{
    ContextAttachResource, ContextCreate, ContextDestroy, ContextDetachResource,
};
use crate::virtio_gpu::protocol::requests::standard::{ResourceAssignUuid, ResourceUnref};
use crate::virtio_gpu::protocol::requests::submit::Submit3D;
use crate::virtio_gpu::protocol::responses::{RespOkNoData, RespResourceUuid};
use crate::virtio_gpu::transport::memory::GuestMemory;

use super::virgl::{CompletedFence, VirglVenusBackend};
use super::{VenusDispatchError, VenusResponse, VenusState};

#[derive(Debug)]
pub enum VenusRuntimeError {
    Backend(virglrenderer::VirglError),
    Dispatch(VenusDispatchError),
    State(super::VenusStateError),
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

impl From<super::VenusStateError> for VenusRuntimeError {
    fn from(value: super::VenusStateError) -> Self {
        Self::State(value)
    }
}

pub struct VenusRuntime {
    pub state: VenusState,
    pub backend: VirglVenusBackend,
}

impl VenusRuntime {
    pub fn new() -> Result<Self, VenusRuntimeError> {
        let backend = VirglVenusBackend::new()?;
        let (version, size) = backend.get_capset_info();
        let mut state = VenusState::new();
        state.capset_version = version;
        state.capset_size = size;
        Ok(Self { state, backend })
    }

    fn ok(header: CtrlHeader) -> VenusResponse {
        VenusResponse {
            bytes: RespOkNoData {
                header: CtrlHeader {
                    typ: RESP_OK_NODATA,
                    ..header
                },
            }
            .encode_le(),
            fence: (header.flags & FLAG_FENCE != 0).then_some(header.fence_id),
        }
    }

    fn capset_info(&self, header: CtrlHeader) -> VenusResponse {
        let (version, size) = self.backend.get_capset_info();
        let mut bytes = Vec::with_capacity(40);
        bytes.extend_from_slice(
            &CtrlHeader {
                typ: RESP_OK_CAPSET_INFO,
                ..header
            }
            .encode_le(),
        );
        bytes.extend_from_slice(&CAPSET_VENUS.to_le_bytes());
        bytes.extend_from_slice(&version.to_le_bytes());
        bytes.extend_from_slice(&size.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        VenusResponse {
            bytes,
            fence: (header.flags & FLAG_FENCE != 0).then_some(header.fence_id),
        }
    }

    fn capset(&self, header: CtrlHeader, version: u32) -> VenusResponse {
        let data = self.backend.get_capset(version);
        let mut bytes = Vec::with_capacity(24 + data.len());
        bytes.extend_from_slice(
            &CtrlHeader {
                typ: RESP_OK_CAPSET,
                ..header
            }
            .encode_le(),
        );
        bytes.extend_from_slice(&data);
        VenusResponse {
            bytes,
            fence: (header.flags & FLAG_FENCE != 0).then_some(header.fence_id),
        }
    }

    pub fn dispatch(
        &mut self,
        request: &[u8],
        guest_memory: &GuestMemory,
    ) -> Result<VenusResponse, VenusRuntimeError> {
        let header = CtrlHeader::decode_le(request).ok_or(VenusDispatchError::InvalidRequest)?;
        match header.typ {
            CMD_GET_CAPSET_INFO => {
                let req =
                    GetCapsetInfo::decode_le(request).ok_or(VenusDispatchError::InvalidRequest)?;
                if req.capset_index != 0 {
                    return Err(VenusDispatchError::State(
                        super::VenusStateError::UnsupportedCapability,
                    )
                    .into());
                }
                Ok(self.capset_info(header))
            }
            CMD_GET_CAPSET => {
                let req =
                    GetCapset::decode_le(request).ok_or(VenusDispatchError::InvalidRequest)?;
                if req.capset_id != CAPSET_VENUS || req.capset_version == 0 {
                    return Err(VenusDispatchError::State(
                        super::VenusStateError::UnsupportedCapability,
                    )
                    .into());
                }
                Ok(self.capset(header, req.capset_version))
            }
            CMD_CTX_CREATE => {
                let req =
                    ContextCreate::decode_le(request).ok_or(VenusDispatchError::InvalidRequest)?;
                if !req.is_venus() {
                    return Err(VenusDispatchError::State(
                        super::VenusStateError::UnsupportedCapability,
                    )
                    .into());
                }
                let name_len = req.nlen.min(64) as usize;
                self.state.create_context(
                    req.header.ctx_id,
                    CAPSET_VENUS,
                    &req.debug_name[..name_len],
                )?;
                if let Err(err) = self.backend.create_context(
                    req.header.ctx_id,
                    req.context_init,
                    std::str::from_utf8(&req.debug_name[..name_len]).ok(),
                ) {
                    self.state.contexts.remove(&req.header.ctx_id);
                    return Err(err.into());
                }
                Ok(Self::ok(header))
            }
            CMD_CTX_DESTROY => {
                let req =
                    ContextDestroy::decode_le(request).ok_or(VenusDispatchError::InvalidRequest)?;
                self.state.destroy_context(req.header.ctx_id)?;
                self.backend.destroy_context(req.header.ctx_id);
                Ok(Self::ok(header))
            }
            CMD_CTX_ATTACH_RESOURCE => {
                let req = ContextAttachResource::decode_le(request)
                    .ok_or(VenusDispatchError::InvalidRequest)?;
                self.state
                    .attach_resource(req.header.ctx_id, req.resource_id)?;
                self.backend
                    .attach_resource(req.header.ctx_id, req.resource_id);
                Ok(Self::ok(header))
            }
            CMD_CTX_DETACH_RESOURCE => {
                let req = ContextDetachResource::decode_le(request)
                    .ok_or(VenusDispatchError::InvalidRequest)?;
                self.state
                    .detach_resource(req.header.ctx_id, req.resource_id)?;
                self.backend
                    .detach_resource(req.header.ctx_id, req.resource_id);
                Ok(Self::ok(header))
            }
            CMD_RESOURCE_CREATE_BLOB => {
                let req = ResourceCreateBlob::decode_le(request)
                    .ok_or(VenusDispatchError::InvalidRequest)?;
                let entry_bytes = (req.nr_entries as usize)
                    .checked_mul(MemEntry::SIZE)
                    .ok_or(VenusDispatchError::InvalidRequest)?;
                let end = ResourceCreateBlob::SIZE
                    .checked_add(entry_bytes)
                    .ok_or(VenusDispatchError::InvalidRequest)?;
                if request.len() < end {
                    return Err(VenusDispatchError::InvalidRequest.into());
                }

                let mut entries = Vec::with_capacity(req.nr_entries as usize);
                let mut guest_size = 0u64;
                for chunk in request[ResourceCreateBlob::SIZE..end].chunks_exact(MemEntry::SIZE) {
                    let entry =
                        MemEntry::decode_le(chunk).ok_or(VenusDispatchError::InvalidRequest)?;
                    guest_size = guest_size
                        .checked_add(u64::from(entry.length))
                        .ok_or(VenusDispatchError::InvalidRequest)?;
                    entries.push(entry);
                }

                self.state.create_blob(
                    req.resource_id,
                    req.blob_id,
                    req.size,
                    req.blob_mem,
                    req.blob_flags,
                    guest_size,
                )?;

                if let Err(err) = self.backend.create_blob(
                    req.header.ctx_id,
                    0,
                    0,
                    req.resource_id,
                    req.blob_mem,
                    req.blob_flags,
                    req.blob_id,
                    req.size,
                    guest_memory,
                    &entries,
                ) {
                    self.state.resources.remove(&req.resource_id);
                    return Err(err.into());
                }
                Ok(Self::ok(header))
            }
            CMD_RESOURCE_UNREF => {
                let req =
                    ResourceUnref::decode_le(request).ok_or(VenusDispatchError::InvalidRequest)?;
                let resource = self
                    .state
                    .resources
                    .get(&req.resource_id)
                    .ok_or(VenusStateError::InvalidResource)?;
                if !resource.attached_contexts.is_empty() {
                    return Err(VenusStateError::ResourceInUse.into());
                }
                self.backend.unref_resource(req.resource_id);
                self.state.unref_resource(req.resource_id)?;
                Ok(Self::ok(header))
            }
            CMD_RESOURCE_MAP_BLOB => {
                let req = ResourceMapBlob::decode_le(request)
                    .ok_or(VenusDispatchError::InvalidRequest)?;
                self.state
                    .resources
                    .get_mut(&req.resource_id)
                    .ok_or(VenusStateError::InvalidResource)?
                    .map(req.offset)?;
                let mut bytes = Vec::with_capacity(32);
                bytes.extend_from_slice(
                    &CtrlHeader {
                        typ: RESP_OK_MAP_INFO,
                        ..header
                    }
                    .encode_le(),
                );
                bytes.extend_from_slice(&SHM_ID_HOST_VISIBLE.to_le_bytes());
                bytes.extend_from_slice(&0u32.to_le_bytes());
                Ok(VenusResponse {
                    bytes,
                    fence: (header.flags & FLAG_FENCE != 0).then_some(header.fence_id),
                })
            }
            CMD_RESOURCE_UNMAP_BLOB => {
                let req = ResourceUnmapBlob::decode_le(request)
                    .ok_or(VenusDispatchError::InvalidRequest)?;
                self.backend.unmap_resource(req.resource_id)?;
                self.state
                    .resources
                    .get_mut(&req.resource_id)
                    .ok_or(VenusStateError::InvalidResource)?
                    .unmap()?;
                Ok(Self::ok(header))
            }
            CMD_RESOURCE_ASSIGN_UUID => {
                let req = ResourceAssignUuid::decode_le(request)
                    .ok_or(VenusDispatchError::InvalidRequest)?;
                let mut x = req.resource_id as u64 ^ 0x9e37_79b9_7f4a_7c15;
                let mut uuid = [0u8; 16];
                for chunk in uuid.chunks_exact_mut(8) {
                    x ^= x >> 12;
                    x ^= x << 25;
                    x ^= x >> 27;
                    x = x.wrapping_mul(0x2545_f491_4f6c_dd1d);
                    chunk.copy_from_slice(&x.to_le_bytes());
                }
                uuid[6] = (uuid[6] & 0x0f) | 0x40;
                uuid[8] = (uuid[8] & 0x3f) | 0x80;
                self.state
                    .resources
                    .get_mut(&req.resource_id)
                    .ok_or(VenusStateError::InvalidResource)?
                    .assign_uuid(uuid)?;
                let response = RespResourceUuid {
                    header: CtrlHeader {
                        typ: RESP_OK_RESOURCE_UUID,
                        ..header
                    },
                    uuid,
                };
                let mut bytes = Vec::with_capacity(40);
                bytes.extend_from_slice(&response.header.encode_le());
                bytes.extend_from_slice(&response.uuid);
                Ok(VenusResponse {
                    bytes,
                    fence: (header.flags & FLAG_FENCE != 0).then_some(header.fence_id),
                })
            }
            CMD_SUBMIT_3D => {
                let req = Submit3D::decode_le(request).ok_or(VenusDispatchError::InvalidRequest)?;
                let fence_bytes = (req.num_in_fences as usize)
                    .checked_mul(8)
                    .ok_or(VenusDispatchError::InvalidRequest)?;
                let command_begin = Submit3D::SIZE
                    .checked_add(fence_bytes)
                    .ok_or(VenusDispatchError::InvalidRequest)?;
                let command_end = command_begin
                    .checked_add(req.size as usize)
                    .ok_or(VenusDispatchError::InvalidRequest)?;
                if command_end > request.len() || req.size % 4 != 0 {
                    return Err(VenusDispatchError::InvalidRequest.into());
                }
                let mut in_fences = Vec::with_capacity(req.num_in_fences as usize);
                for i in 0..req.num_in_fences as usize {
                    let start = Submit3D::SIZE + i * 8;
                    in_fences.push(u64::from_le_bytes(
                        request[start..start + 8]
                            .try_into()
                            .expect("fixed-size slice"),
                    ));
                }
                let ring = if req.header.flags & FLAG_INFO_RING_IDX != 0 {
                    req.header.ring_idx
                } else {
                    0
                };
                let point = self.state.submit(
                    req.header.ctx_id,
                    ring,
                    &in_fences,
                    &request[command_begin..command_end],
                )?;
                let mut commands = request[command_begin..command_end].to_vec();
                if let Err(err) = self.backend.submit(
                    req.header.ctx_id,
                    req.header.flags,
                    ring,
                    point.id,
                    &mut commands,
                    &in_fences,
                ) {
                    return Err(err.into());
                }
                let mut response = Self::ok(header);
                response.fence = Some(point.id);
                Ok(response)
            }
            _ => Err(VenusDispatchError::UnsupportedCommand.into()),
        }
    }

    pub fn poll_fences(&self) -> Vec<CompletedFence> {
        self.backend.poll()
    }
}
