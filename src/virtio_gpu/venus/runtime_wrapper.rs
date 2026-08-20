#![cfg(feature = "virglrenderer-backend")]

use std::collections::HashMap;

use crate::virtio_gpu::protocol::commands::{CMD_SUBMIT_3D, FLAG_FENCE, FLAG_INFO_RING_IDX};
use crate::virtio_gpu::protocol::header::CtrlHeader;
use crate::virtio_gpu::protocol::requests::submit::Submit3D;
use crate::virtio_gpu::transport::memory::GuestMemory;

use super::runtime_impl::{VenusRuntime as InnerRuntime, VenusRuntimeError};
use super::virgl::CompletedFence;
use super::{VenusDispatchError, VenusResponse};

pub struct VenusRuntime {
    inner: InnerRuntime,
    guest_to_internal: HashMap<(u32, u8, u64), u64>,
    internal_to_guest: HashMap<(u32, u8, u64), u64>,
}

impl VenusRuntime {
    pub fn new(guest_memory: GuestMemory) -> Result<Self, VenusRuntimeError> {
        Ok(Self {
            inner: InnerRuntime::new(guest_memory)?,
            guest_to_internal: HashMap::new(),
            internal_to_guest: HashMap::new(),
        })
    }

    pub fn dispatch(&mut self, request: &[u8]) -> Result<VenusResponse, VenusRuntimeError> {
        let header = CtrlHeader::decode_le(request).ok_or(VenusDispatchError::InvalidRequest)?;
        if header.typ != CMD_SUBMIT_3D || header.flags & FLAG_FENCE == 0 {
            return self.inner.dispatch(request);
        }

        let submit = Submit3D::decode_le(request).ok_or(VenusDispatchError::InvalidRequest)?;
        let ring = if header.flags & FLAG_INFO_RING_IDX != 0 {
            header.ring_idx
        } else {
            0
        };
        let fence_bytes = (submit.num_in_fences as usize)
            .checked_mul(8)
            .ok_or(VenusDispatchError::InvalidRequest)?;
        let begin = Submit3D::SIZE;
        let end = begin
            .checked_add(fence_bytes)
            .ok_or(VenusDispatchError::InvalidRequest)?;
        if request.len() < end {
            return Err(VenusDispatchError::InvalidRequest.into());
        }

        let mut translated = request.to_vec();
        for index in 0..submit.num_in_fences as usize {
            let offset = begin + index * 8;
            let guest_id = u64::from_le_bytes(
                request[offset..offset + 8]
                    .try_into()
                    .map_err(|_| VenusDispatchError::InvalidRequest)?,
            );
            if let Some(&internal_id) = self.guest_to_internal.get(&(header.ctx_id, ring, guest_id))
            {
                translated[offset..offset + 8].copy_from_slice(&internal_id.to_le_bytes());
            }
        }

        let mut response = self.inner.dispatch(&translated)?;
        let internal_fence = response.fence.ok_or(VenusDispatchError::InvalidRequest)?;
        self.guest_to_internal
            .insert((header.ctx_id, ring, header.fence_id), internal_fence);
        self.internal_to_guest
            .insert((header.ctx_id, ring, internal_fence), header.fence_id);

        // The VirtIO protocol requires the guest supplied fence_id to be copied
        // to the response rather than exposing the host-side sequence number.
        response.bytes[8..16].copy_from_slice(&header.fence_id.to_le_bytes());
        response.fence = Some(header.fence_id);

        // Completed fences older than this timeline point can no longer be
        // referenced by a future unsignaled fence on the same timeline.
        self.guest_to_internal.retain(|(ctx, r, guest), _| {
            *ctx != header.ctx_id || *r != ring || *guest >= header.fence_id
        });

        Ok(response)
    }

    pub fn poll_fences(&mut self) -> Vec<CompletedFence> {
        self.inner
            .poll_fences()
            .into_iter()
            .map(|mut fence| {
                if let Some(&guest_id) =
                    self.internal_to_guest
                        .get(&(fence.ctx_id, fence.ring_idx, fence.fence_id))
                {
                    fence.fence_id = guest_id;
                    self.internal_to_guest.remove(&(
                        fence.ctx_id,
                        fence.ring_idx,
                        self.guest_to_internal
                            .get(&(fence.ctx_id, fence.ring_idx, guest_id))
                            .copied()
                            .unwrap_or(0),
                    ));
                }
                fence
            })
            .collect()
    }
}
