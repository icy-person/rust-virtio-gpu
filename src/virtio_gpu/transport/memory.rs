use std::sync::{Arc, RwLock};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GuestAddress(pub u64);

impl GuestAddress {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn offset(self, offset: usize) -> Self {
        Self(self.0 + offset as u64)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum GuestMemoryError {
    OutOfBounds,
    AddressOverflow,
}

/// Fixed-size guest memory shared by all device components.
///
/// The backing allocation is a boxed slice and is never resized after creation.
/// This gives integrations such as virglrenderer a stable address for an I/O vector
/// while the allocation remains owned by the `GuestMemory` instance.
#[derive(Clone)]
pub struct GuestMemory {
    base: GuestAddress,
    data: Arc<RwLock<Box<[u8]>>>,
}

impl GuestMemory {
    pub fn new(base: GuestAddress, size: usize) -> Self {
        Self {
            base,
            data: Arc::new(RwLock::new(vec![0; size].into_boxed_slice())),
        }
    }

    pub fn base(&self) -> GuestAddress {
        self.base
    }

    pub fn len(&self) -> usize {
        self.data.read().expect("guest memory poisoned").len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn offset(&self, addr: GuestAddress, len: usize) -> Result<usize, GuestMemoryError> {
        let offset = addr
            .0
            .checked_sub(self.base.0)
            .ok_or(GuestMemoryError::OutOfBounds)?;

        let offset = usize::try_from(offset).map_err(|_| GuestMemoryError::AddressOverflow)?;

        let end = offset
            .checked_add(len)
            .ok_or(GuestMemoryError::AddressOverflow)?;

        if end > self.len() {
            return Err(GuestMemoryError::OutOfBounds);
        }

        Ok(offset)
    }

    /// Returns a stable mutable pointer into the fixed guest allocation.
    ///
    /// The caller must keep this `GuestMemory` alive and must not expose the pointer
    /// outside the allocation validated by this method. The allocation is never resized.
    pub fn as_mut_ptr(&self, addr: GuestAddress, len: usize) -> Result<*mut u8, GuestMemoryError> {
        let offset = self.offset(addr, len)?;
        let memory = self.data.read().expect("guest memory poisoned");
        // The boxed slice is fixed-size for the lifetime of this GuestMemory.
        Ok(unsafe { memory.as_ptr().add(offset) as *mut u8 })
    }

    pub fn read(&self, addr: GuestAddress, out: &mut [u8]) -> Result<(), GuestMemoryError> {
        let offset = self.offset(addr, out.len())?;

        let memory = self.data.read().expect("guest memory poisoned");

        out.copy_from_slice(&memory[offset..offset + out.len()]);

        Ok(())
    }

    pub fn write(&self, addr: GuestAddress, data: &[u8]) -> Result<(), GuestMemoryError> {
        let offset = self.offset(addr, data.len())?;

        let mut memory = self.data.write().expect("guest memory poisoned");

        memory[offset..offset + data.len()].copy_from_slice(data);

        Ok(())
    }

    pub fn read_slice(&self, addr: GuestAddress, len: usize) -> Result<Vec<u8>, GuestMemoryError> {
        let mut buffer = vec![0u8; len];

        self.read(addr, &mut buffer)?;

        Ok(buffer)
    }

    pub fn write_slice(&self, addr: GuestAddress, data: &[u8]) -> Result<(), GuestMemoryError> {
        self.write(addr, data)
    }

    pub fn read_u16(&self, addr: GuestAddress) -> Result<u16, GuestMemoryError> {
        let mut bytes = [0u8; 2];

        self.read(addr, &mut bytes)?;

        Ok(u16::from_le_bytes(bytes))
    }

    pub fn read_u32(&self, addr: GuestAddress) -> Result<u32, GuestMemoryError> {
        let mut bytes = [0u8; 4];

        self.read(addr, &mut bytes)?;

        Ok(u32::from_le_bytes(bytes))
    }

    pub fn read_u64(&self, addr: GuestAddress) -> Result<u64, GuestMemoryError> {
        let mut bytes = [0u8; 8];

        self.read(addr, &mut bytes)?;

        Ok(u64::from_le_bytes(bytes))
    }

    pub fn write_u16(&self, addr: GuestAddress, value: u16) -> Result<(), GuestMemoryError> {
        self.write(addr, &value.to_le_bytes())
    }

    pub fn write_u32(&self, addr: GuestAddress, value: u32) -> Result<(), GuestMemoryError> {
        self.write(addr, &value.to_le_bytes())
    }

    pub fn write_u64(&self, addr: GuestAddress, value: u64) -> Result<(), GuestMemoryError> {
        self.write(addr, &value.to_le_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_round_trip() {
        let memory = GuestMemory::new(GuestAddress::new(0x1000), 64);
        let data = [1, 2, 3, 4, 5];
        memory.write(GuestAddress::new(0x1010), &data).unwrap();
        let mut out = [0u8; 5];
        memory.read(GuestAddress::new(0x1010), &mut out).unwrap();
        assert_eq!(out, data);
    }

    #[test]
    fn memory_rejects_before_base() {
        let memory = GuestMemory::new(GuestAddress::new(0x1000), 64);
        let mut out = [0u8; 1];
        assert_eq!(
            memory.read(GuestAddress::new(0x0fff), &mut out),
            Err(GuestMemoryError::OutOfBounds)
        );
    }

    #[test]
    fn memory_rejects_past_end() {
        let memory = GuestMemory::new(GuestAddress::new(0x1000), 64);
        let mut out = [0u8; 1];
        assert_eq!(
            memory.read(GuestAddress::new(0x1040), &mut out),
            Err(GuestMemoryError::OutOfBounds)
        );
    }

    #[test]
    fn integer_access_is_little_endian() {
        let memory = GuestMemory::new(GuestAddress::new(0x1000), 32);
        memory
            .write_u32(GuestAddress::new(0x1004), 0x11223344)
            .unwrap();
        assert_eq!(
            memory.read_u32(GuestAddress::new(0x1004)).unwrap(),
            0x11223344
        );
    }

    #[test]
    fn cloned_memory_shares_guest_memory() {
        let memory = GuestMemory::new(GuestAddress::new(0x1000), 64);
        let shared = memory.clone();
        memory
            .write(GuestAddress::new(0x1010), &[1, 2, 3, 4])
            .unwrap();
        let mut out = [0u8; 4];
        shared.read(GuestAddress::new(0x1010), &mut out).unwrap();
        assert_eq!(out, [1, 2, 3, 4]);
    }

    #[test]
    fn pointer_access_is_bounds_checked() {
        let memory = GuestMemory::new(GuestAddress::new(0x1000), 64);
        assert!(memory.as_mut_ptr(GuestAddress::new(0x1010), 16).is_ok());
        assert_eq!(
            memory.as_mut_ptr(GuestAddress::new(0x1040), 1),
            Err(GuestMemoryError::OutOfBounds)
        );
    }

    #[test]
    fn address_overflow_is_rejected() {
        let memory = GuestMemory::new(GuestAddress::new(0x1000), 64);
        let mut out = [0u8; 8];
        assert_eq!(
            memory.read(GuestAddress::new(u64::MAX), &mut out),
            Err(GuestMemoryError::OutOfBounds)
        );
    }
}
