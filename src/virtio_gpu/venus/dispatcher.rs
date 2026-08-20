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
use crate::virtio_gpu::protocol::responses::{RespMapInfo, RespOkNoData, RespResourceUuid};

use super::{VenusState, VenusStateError};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VenusResponse {
    pub bytes: Vec<u8>,
    pub fence: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VenusDispatchError {
    InvalidRequest,
    UnsupportedCommand,
    State(VenusStateError),
}

impl From<VenusStateError> for VenusDispatchError {
    fn from(value: VenusStateError) -> Self {
        Self::State(value)
    }
}

fn ok_header(request: CtrlHeader) -> CtrlHeader {
    CtrlHeader {
        typ: RESP_OK_NODATA,
        flags: request.flags,
        fence_id: request.fence_id,
        ctx_id: request.ctx_id,
        ring_idx: request.ring_idx,
        padding: [0; 3],
    }
}

fn nodata(request: CtrlHeader) -> VenusResponse {
    VenusResponse {
        bytes: RespOkNoData {
            header: ok_header(request),
        }
        .encode_le(),
        fence: (request.flags & FLAG_FENCE != 0).then_some(request.fence_id),
    }
}

fn read_entries(bytes: &[u8], count: u32) -> Result<u64, VenusDispatchError> {
    let count = usize::try_from(count).map_err(|_| VenusDispatchError::InvalidRequest)?;
    let expected = count
        .checked_mul(MemEntry::SIZE)
        .ok_or(VenusDispatchError::InvalidRequest)?;
    if bytes.len() < expected {
        return Err(VenusDispatchError::InvalidRequest);
    }

    let mut total = 0u64;
    for index in 0..count {
        let start = index * MemEntry::SIZE;
        let entry = MemEntry::decode_le(&bytes[start..start + MemEntry::SIZE])
            .ok_or(VenusDispatchError::InvalidRequest)?;
        total = total
            .checked_add(u64::from(entry.length))
            .ok_or(VenusDispatchError::InvalidRequest)?;
    }
    Ok(total)
}

fn assign_uuid(resource_id: u32) -> [u8; 16] {
    let mut x = (resource_id as u64) ^ 0x9e37_79b9_7f4a_7c15;
    let mut out = [0u8; 16];
    for chunk in out.chunks_exact_mut(8) {
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        x = x.wrapping_mul(0x2545_f491_4f6c_dd1d);
        chunk.copy_from_slice(&x.to_le_bytes());
    }
    out[6] = (out[6] & 0x0f) | 0x40;
    out[8] = (out[8] & 0x3f) | 0x80;
    out
}

impl VenusState {
    pub fn dispatch(&mut self, raw_request: &[u8]) -> Result<VenusResponse, VenusDispatchError> {
        let header = CtrlHeader::decode_le(raw_request).ok_or(VenusDispatchError::InvalidRequest)?;

        match header.typ {
            CMD_CTX_CREATE => {
                let request = ContextCreate::decode_le(raw_request)
                    .ok_or(VenusDispatchError::InvalidRequest)?;
                let name_len = usize::try_from(request.nlen.min(64))
                    .map_err(|_| VenusDispatchError::InvalidRequest)?;
                self.create_context(
                    request.header.ctx_id,
                    request.capset_id(),
                    &request.debug_name[..name_len],
                )?;
                Ok(nodata(header))
            }
            CMD_CTX_DESTROY => {
                let request = ContextDestroy::decode_le(raw_request)
                    .ok_or(VenusDispatchError::InvalidRequest)?;
                self.destroy_context(request.header.ctx_id)?;
                Ok(nodata(header))
            }
            CMD_CTX_ATTACH_RESOURCE => {
                let request = ContextAttachResource::decode_le(raw_request)
                    .ok_or(VenusDispatchError::InvalidRequest)?;
                self.attach_resource(request.header.ctx_id, request.resource_id)?;
                Ok(nodata(header))
            }
            CMD_CTX_DETACH_RESOURCE => {
                let request = ContextDetachResource::decode_le(raw_request)
                    .ok_or(VenusDispatchError::InvalidRequest)?;
                self.detach_resource(request.header.ctx_id, request.resource_id)?;
                Ok(nodata(header))
            }
            CMD_RESOURCE_CREATE_BLOB => {
                let request = ResourceCreateBlob::decode_le(raw_request)
                    .ok_or(VenusDispatchError::InvalidRequest)?;
                let entry_bytes = request
                    .nr_entries
                    .checked_mul(MemEntry::SIZE as u32)
                    .ok_or(VenusDispatchError::InvalidRequest)?
                    as usize;
                let end = ResourceCreateBlob::SIZE
                    .checked_add(entry_bytes)
                    .ok_or(VenusDispatchError::InvalidRequest)?;
                if raw_request.len() < end {
                    return Err(VenusDispatchError::InvalidRequest);
                }
                let guest_backing_size = read_entries(
                    &raw_request[ResourceCreateBlob::SIZE..end],
                    request.nr_entries,
                )?;
                self.create_blob(
                    request.resource_id,
                    request.blob_id,
                    request.size,
                    request.blob_mem,
                    request.blob_flags,
                    guest_backing_size,
                )?;
                Ok(nodata(header))
            }
            CMD_RESOURCE_UNREF => {
                let request = ResourceUnref::decode_le(raw_request)
                    .ok_or(VenusDispatchError::InvalidRequest)?;
                self.unref_resource(request.resource_id)?;
                Ok(nodata(header))
            }
            CMD_RESOURCE_MAP_BLOB => {
                let request = ResourceMapBlob::decode_le(raw_request)
                    .ok_or(VenusDispatchError::InvalidRequest)?;
                self.resources
                    .get_mut(&request.resource_id)
                    .ok_or(VenusStateError::InvalidResource)?
                    .map(request.offset)?;

                let response = RespMapInfo {
                    header: CtrlHeader {
                        typ: RESP_OK_MAP_INFO,
                        flags: header.flags,
                        fence_id: header.fence_id,
                        ctx_id: header.ctx_id,
                        ring_idx: header.ring_idx,
                        padding: [0; 3],
                    },
                    map_info: SHM_ID_HOST_VISIBLE,
                    padding: 0,
                };
                Ok(VenusResponse {
                    bytes: {
                        let mut bytes = response.header.encode_le().to_vec();
                        bytes.extend_from_slice(&response.map_info.to_le_bytes());
                        bytes.extend_from_slice(&response.padding.to_le_bytes());
                        bytes
                    },
                    fence: (header.flags & FLAG_FENCE != 0).then_some(header.fence_id),
                })
            }
            CMD_RESOURCE_UNMAP_BLOB => {
                let request = ResourceUnmapBlob::decode_le(raw_request)
                    .ok_or(VenusDispatchError::InvalidRequest)?;
                self.resources
                    .get_mut(&request.resource_id)
                    .ok_or(VenusStateError::InvalidResource)?
                    .unmap()?;
                Ok(nodata(header))
            }
            CMD_RESOURCE_ASSIGN_UUID => {
                let request = ResourceAssignUuid::decode_le(raw_request)
                    .ok_or(VenusDispatchError::InvalidRequest)?;
                let uuid = assign_uuid(request.resource_id);
                self.resources
                    .get_mut(&request.resource_id)
                    .ok_or(VenusStateError::InvalidResource)?
                    .assign_uuid(uuid)?;
                let response = RespResourceUuid {
                    header: CtrlHeader {
                        typ: RESP_OK_RESOURCE_UUID,
                        flags: header.flags,
                        fence_id: header.fence_id,
                        ctx_id: header.ctx_id,
                        ring_idx: header.ring_idx,
                        padding: [0; 3],
                    },
                    uuid,
                };
                Ok(VenusResponse {
                    bytes: {
                        let mut bytes = response.header.encode_le().to_vec();
                        bytes.extend_from_slice(&response.uuid);
                        bytes
                    },
                    fence: (header.flags & FLAG_FENCE != 0).then_some(header.fence_id),
                })
            }
            CMD_SUBMIT_3D => {
                let request = Submit3D::decode_le(raw_request)
                    .ok_or(VenusDispatchError::InvalidRequest)?;
                let in_fence_bytes = (request.num_in_fences as usize)
                    .checked_mul(8)
                    .ok_or(VenusDispatchError::InvalidRequest)?;
                let fence_begin = Submit3D::SIZE;
                let command_begin = fence_begin
                    .checked_add(in_fence_bytes)
                    .ok_or(VenusDispatchError::InvalidRequest)?;
                let command_end = command_begin
                    .checked_add(request.size as usize)
                    .ok_or(VenusDispatchError::InvalidRequest)?;
                if raw_request.len() < command_end {
                    return Err(VenusDispatchError::InvalidRequest);
                }

                let mut in_fences = Vec::with_capacity(request.num_in_fences as usize);
                for chunk in raw_request[fence_begin..command_begin].chunks_exact(8) {
                    in_fences.push(u64::from_le_bytes(
                        chunk
                            .try_into()
                            .map_err(|_| VenusDispatchError::InvalidRequest)?,
                    ));
                }

                let ring = if request.header.flags & FLAG_INFO_RING_IDX != 0 {
                    request.header.ring_idx
                } else {
                    0
                };
                let point = self.submit(
                    request.header.ctx_id,
                    ring,
                    &in_fences,
                    &raw_request[command_begin..command_end],
                )?;
                let mut response = nodata(header);
                response.fence = Some(point.id);
                Ok(response)
            }
            CMD_GET_CAPSET_INFO => {
                let request = GetCapsetInfo::decode_le(raw_request)
                    .ok_or(VenusDispatchError::InvalidRequest)?;
                if request.capset_index != 0 {
                    return Err(VenusDispatchError::State(
                        VenusStateError::UnsupportedCapability,
                    ));
                }
                let response_header = CtrlHeader {
                    typ: RESP_OK_CAPSET_INFO,
                    flags: header.flags,
                    fence_id: header.fence_id,
                    ctx_id: header.ctx_id,
                    ring_idx: header.ring_idx,
                    padding: [0; 3],
                };
                let mut bytes = Vec::with_capacity(40);
                bytes.extend_from_slice(&response_header.encode_le());
                bytes.extend_from_slice(&CAPSET_VENUS.to_le_bytes());
                bytes.extend_from_slice(&self.capset_version.to_le_bytes());
                bytes.extend_from_slice(&self.capset_size.to_le_bytes());
                bytes.extend_from_slice(&0u32.to_le_bytes());
                Ok(VenusResponse {
                    bytes,
                    fence: (header.flags & FLAG_FENCE != 0).then_some(header.fence_id),
                })
            }
            CMD_GET_CAPSET => {
                let request = GetCapset::decode_le(raw_request)
                    .ok_or(VenusDispatchError::InvalidRequest)?;
                if request.capset_id != CAPSET_VENUS || request.capset_version == 0 {
                    return Err(VenusDispatchError::State(
                        VenusStateError::UnsupportedCapability,
                    ));
                }
                let response_header = CtrlHeader {
                    typ: RESP_OK_CAPSET,
                    flags: header.flags,
                    fence_id: header.fence_id,
                    ctx_id: header.ctx_id,
                    ring_idx: header.ring_idx,
                    padding: [0; 3],
                };
                let payload = venus_capset_payload();
                let mut bytes = Vec::with_capacity(24 + payload.len());
                bytes.extend_from_slice(&response_header.encode_le());
                bytes.extend_from_slice(&payload);
                Ok(VenusResponse {
                    bytes,
                    fence: (header.flags & FLAG_FENCE != 0).then_some(header.fence_id),
                })
            }
            _ => Err(VenusDispatchError::UnsupportedCommand),
        }
    }
}

fn venus_capset_payload() -> Vec<u8> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_commands_round_trip_through_dispatcher() {
        let mut state = VenusState::new();
        let request = ContextCreate::new(7, CAPSET_VENUS, b"ctx").encode_le();
        let response = state.dispatch(&request).unwrap();
        assert_eq!(
            CtrlHeader::decode_le(&response.bytes).unwrap().typ,
            RESP_OK_NODATA
        );
        assert!(state.contexts.contains_key(&7));
    }

    #[test]
    fn invalid_submit_is_rejected_without_state_mutation() {
        let mut state = VenusState::new();
        state.create_context(1, CAPSET_VENUS, b"ctx").unwrap();
        let request = Submit3D::new(1, 3).encode_le();
        assert_eq!(
            state.dispatch(&request),
            Err(VenusDispatchError::InvalidRequest)
        );
        assert_eq!(state.fences.completed(), 0);
    }

    #[test]
    fn submit_uses_global_fence_id() {
        let mut state = VenusState::new();
        state.create_context(1, CAPSET_VENUS, b"ctx").unwrap();
        let request = Submit3D::new(1, 4);
        let mut bytes = request.encode_le().to_vec();
        bytes.extend_from_slice(&[0; 4]);
        let response = state.dispatch(&bytes).unwrap();
        assert_eq!(response.fence, Some(1));
        assert_eq!(state.fences.completed(), 0);
    }
}
