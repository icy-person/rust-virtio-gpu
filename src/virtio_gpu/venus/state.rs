use std::collections::{BTreeMap, BTreeSet};

use crate::virtio_gpu::protocol::commands::{
    BLOB_FLAG_USE_CROSS_DEVICE, BLOB_FLAG_USE_MAPPABLE, BLOB_FLAG_USE_SHAREABLE,
    BLOB_MEM_GUEST, BLOB_MEM_HOST3D, BLOB_MEM_HOST3D_GUEST, CAPSET_VENUS,
};

pub const VENUS_MAX_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VenusStateError {
    InvalidContext,
    ContextAlreadyExists,
    ResourceAlreadyExists,
    InvalidResource,
    ResourceInUse,
    InvalidBlobMemory,
    InvalidBlobFlags,
    InvalidBlobSize,
    InvalidMapOffset,
    AlreadyMapped,
    NotMapped,
    InvalidCommandStream,
    UnsupportedCapability,
    InvalidRing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlobMemory {
    Guest,
    Host3d,
    Host3dGuest,
}

impl BlobMemory {
    pub fn from_wire(value: u32) -> Result<Self, VenusStateError> {
        match value {
            BLOB_MEM_GUEST => Ok(Self::Guest),
            BLOB_MEM_HOST3D => Ok(Self::Host3d),
            BLOB_MEM_HOST3D_GUEST => Ok(Self::Host3dGuest),
            _ => Err(VenusStateError::InvalidBlobMemory),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VenusResource {
    pub id: u32,
    pub blob_id: u64,
    pub size: u64,
    pub memory: BlobMemory,
    pub mappable: bool,
    pub shareable: bool,
    pub cross_device: bool,
    pub guest_backing_size: u64,
    pub mapped_offset: Option<u64>,
    pub uuid: Option<[u8; 16]>,
    pub attached_contexts: BTreeSet<u32>,
}

impl VenusResource {
    pub fn new_blob(
        id: u32,
        blob_id: u64,
        size: u64,
        memory: BlobMemory,
        flags: u32,
        guest_backing_size: u64,
    ) -> Result<Self, VenusStateError> {
        if size == 0 {
            return Err(VenusStateError::InvalidBlobSize);
        }

        let known_flags =
            BLOB_FLAG_USE_MAPPABLE | BLOB_FLAG_USE_SHAREABLE | BLOB_FLAG_USE_CROSS_DEVICE;
        if flags & !known_flags != 0 {
            return Err(VenusStateError::InvalidBlobFlags);
        }

        match memory {
            BlobMemory::Guest | BlobMemory::Host3dGuest if guest_backing_size < size => {
                return Err(VenusStateError::InvalidBlobSize)
            }
            _ => {}
        }

        Ok(Self {
            id,
            blob_id,
            size,
            memory,
            mappable: flags & BLOB_FLAG_USE_MAPPABLE != 0,
            shareable: flags & BLOB_FLAG_USE_SHAREABLE != 0,
            cross_device: flags & BLOB_FLAG_USE_CROSS_DEVICE != 0,
            guest_backing_size,
            mapped_offset: None,
            uuid: None,
            attached_contexts: BTreeSet::new(),
        })
    }

    pub fn map(&mut self, offset: u64) -> Result<u64, VenusStateError> {
        if !self.mappable {
            return Err(VenusStateError::InvalidMapOffset);
        }
        if self.mapped_offset.is_some() {
            return Err(VenusStateError::AlreadyMapped);
        }
        if offset >= self.size {
            return Err(VenusStateError::InvalidMapOffset);
        }

        self.mapped_offset = Some(offset);
        Ok(offset)
    }

    pub fn unmap(&mut self) -> Result<(), VenusStateError> {
        if self.mapped_offset.take().is_none() {
            return Err(VenusStateError::NotMapped);
        }
        Ok(())
    }

    pub fn assign_uuid(&mut self, uuid: [u8; 16]) -> Result<(), VenusStateError> {
        if self.uuid.is_some() {
            return Err(VenusStateError::ResourceAlreadyExists);
        }
        self.uuid = Some(uuid);
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VenusContext {
    pub id: u32,
    pub capset_id: u32,
    pub debug_name: Vec<u8>,
    pub attached_resources: BTreeSet<u32>,
    pub last_submitted_fence: u64,
}

impl VenusContext {
    pub fn new(id: u32, capset_id: u32, debug_name: &[u8]) -> Result<Self, VenusStateError> {
        if capset_id != CAPSET_VENUS {
            return Err(VenusStateError::UnsupportedCapability);
        }
        Ok(Self {
            id,
            capset_id,
            debug_name: debug_name[..debug_name.len().min(64)].to_vec(),
            attached_resources: BTreeSet::new(),
            last_submitted_fence: 0,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FencePoint {
    pub ring: u8,
    pub value: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FenceTracker {
    next: BTreeMap<u8, u64>,
    completed: BTreeMap<u8, u64>,
}

impl FenceTracker {
    pub fn allocate(&mut self, ring: u8) -> Result<FencePoint, VenusStateError> {
        if ring >= 64 {
            return Err(VenusStateError::InvalidRing);
        }
        let value = self.next.get(&ring).copied().unwrap_or(0).saturating_add(1);
        self.next.insert(ring, value);
        Ok(FencePoint { ring, value })
    }

    pub fn signal(&mut self, point: FencePoint) -> Result<(), VenusStateError> {
        if point.ring >= 64 {
            return Err(VenusStateError::InvalidRing);
        }
        let current = self.completed.get(&point.ring).copied().unwrap_or(0);
        if point.value > current {
            self.completed.insert(point.ring, point.value);
        }
        Ok(())
    }

    pub fn completed(&self, ring: u8) -> u64 {
        self.completed.get(&ring).copied().unwrap_or(0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VenusState {
    pub contexts: BTreeMap<u32, VenusContext>,
    pub resources: BTreeMap<u32, VenusResource>,
    pub fences: FenceTracker,
    pub capset_version: u32,
    pub capset_size: u32,
}

impl Default for VenusState {
    fn default() -> Self {
        Self::new()
    }
}

impl VenusState {
    pub const fn new() -> Self {
        Self {
            contexts: BTreeMap::new(),
            resources: BTreeMap::new(),
            fences: FenceTracker {
                next: BTreeMap::new(),
                completed: BTreeMap::new(),
            },
            capset_version: VENUS_MAX_VERSION,
            capset_size: 0,
        }
    }

    pub fn create_context(
        &mut self,
        id: u32,
        capset_id: u32,
        debug_name: &[u8],
    ) -> Result<(), VenusStateError> {
        if id == 0 {
            return Err(VenusStateError::InvalidContext);
        }
        if self.contexts.contains_key(&id) {
            return Err(VenusStateError::ContextAlreadyExists);
        }
        let context = VenusContext::new(id, capset_id, debug_name)?;
        self.contexts.insert(id, context);
        Ok(())
    }

    pub fn destroy_context(&mut self, id: u32) -> Result<(), VenusStateError> {
        let context = self
            .contexts
            .get(&id)
            .ok_or(VenusStateError::InvalidContext)?;
        if !context.attached_resources.is_empty() {
            return Err(VenusStateError::ResourceInUse);
        }
        self.contexts.remove(&id);
        Ok(())
    }

    pub fn attach_resource(&mut self, context_id: u32, resource_id: u32) -> Result<(), VenusStateError> {
        if !self.resources.contains_key(&resource_id) {
            return Err(VenusStateError::InvalidResource);
        }
        let context = self
            .contexts
            .get_mut(&context_id)
            .ok_or(VenusStateError::InvalidContext)?;
        context.attached_resources.insert(resource_id);
        self.resources
            .get_mut(&resource_id)
            .expect("resource checked above")
            .attached_contexts
            .insert(context_id);
        Ok(())
    }

    pub fn detach_resource(&mut self, context_id: u32, resource_id: u32) -> Result<(), VenusStateError> {
        let context = self
            .contexts
            .get_mut(&context_id)
            .ok_or(VenusStateError::InvalidContext)?;
        if !context.attached_resources.remove(&resource_id) {
            return Err(VenusStateError::InvalidResource);
        }
        if let Some(resource) = self.resources.get_mut(&resource_id) {
            resource.attached_contexts.remove(&context_id);
        }
        Ok(())
    }

    pub fn create_blob(
        &mut self,
        id: u32,
        blob_id: u64,
        size: u64,
        memory: u32,
        flags: u32,
        guest_backing_size: u64,
    ) -> Result<(), VenusStateError> {
        if self.resources.contains_key(&id) {
            return Err(VenusStateError::ResourceAlreadyExists);
        }
        let memory = BlobMemory::from_wire(memory)?;
        let resource = VenusResource::new_blob(id, blob_id, size, memory, flags, guest_backing_size)?;
        self.resources.insert(id, resource);
        Ok(())
    }

    pub fn unref_resource(&mut self, id: u32) -> Result<(), VenusStateError> {
        let resource = self
            .resources
            .get(&id)
            .ok_or(VenusStateError::InvalidResource)?;
        if !resource.attached_contexts.is_empty() {
            return Err(VenusStateError::ResourceInUse);
        }
        self.resources.remove(&id);
        Ok(())
    }

    pub fn submit(&mut self, context_id: u32, ring: u8, command_stream: &[u8]) -> Result<FencePoint, VenusStateError> {
        if !self.contexts.contains_key(&context_id) {
            return Err(VenusStateError::InvalidContext);
        }
        if command_stream.len() > (u32::MAX as usize) || command_stream.len() % 4 != 0 {
            return Err(VenusStateError::InvalidCommandStream);
        }

        let point = self.fences.allocate(ring)?;
        self.contexts
            .get_mut(&context_id)
            .expect("context checked above")
            .last_submitted_fence = point.value;
        Ok(point)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blob_lifecycle() {
        let mut state = VenusState::new();
        state
            .create_blob(1, 42, 8192, BLOB_MEM_HOST3D_GUEST, BLOB_FLAG_USE_MAPPABLE, 8192)
            .unwrap();
        assert_eq!(state.resources.len(), 1);
        assert_eq!(state.resources.get(&1).unwrap().map(0x1000).unwrap(), 0x1000);
        assert_eq!(state.resources.get_mut(&1).unwrap().unmap(), Ok(()));
        assert_eq!(state.unref_resource(1), Ok(()));
    }

    #[test]
    fn context_attachment_is_symmetric() {
        let mut state = VenusState::new();
        state.create_context(1, CAPSET_VENUS, b"ctx").unwrap();
        state.create_blob(2, 9, 4096, BLOB_MEM_HOST3D, 0, 0).unwrap();
        state.attach_resource(1, 2).unwrap();
        assert!(state.contexts.get(&1).unwrap().attached_resources.contains(&2));
        assert!(state.resources.get(&2).unwrap().attached_contexts.contains(&1));
        state.detach_resource(1, 2).unwrap();
        assert!(state.contexts.get(&1).unwrap().attached_resources.is_empty());
        assert!(state.resources.get(&2).unwrap().attached_contexts.is_empty());
    }

    #[test]
    fn fences_are_monotonic_per_ring() {
        let mut tracker = FenceTracker::default();
        let a = tracker.allocate(0).unwrap();
        let b = tracker.allocate(0).unwrap();
        assert!(b.value > a.value);
        tracker.signal(b).unwrap();
        tracker.signal(a).unwrap();
        assert_eq!(tracker.completed(0), b.value);
    }

    #[test]
    fn submit_requires_word_aligned_stream() {
        let mut state = VenusState::new();
        state.create_context(7, CAPSET_VENUS, b"venus").unwrap();
        assert!(state.submit(7, 0, &[0; 8]).is_ok());
        assert_eq!(
            state.submit(7, 0, &[0; 6]),
            Err(VenusStateError::InvalidCommandStream)
        );
    }
}
