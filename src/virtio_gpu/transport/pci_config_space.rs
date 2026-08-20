//! Minimal conventional PCI configuration-space model used by the VirtIO PCI
//! transport implementation. It keeps the header, BARs and VirtIO capability
//! list in a serializable 4 KiB configuration-space image.

use crate::virtio_gpu::transport::pci::{
    PciBar, VIRTIO_PCI_CAP_PCI_CFG, VirtioPciCap64, VirtioPciCapability,
    VirtioPciNotifyCapability,
};

/// `VIRTIO_PCI_CAP_PCI_CFG` capability and its four-byte access window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VirtioPciCfgCapability {
    pub cap: VirtioPciCapability,
    pub pci_cfg_data: [u8; 4],
}

impl VirtioPciCfgCapability {
    pub const SIZE: usize = VirtioPciCapability::SIZE + 4;

    pub const fn new(bar: u8, offset: u32, length: u32) -> Self {
        Self {
            cap: VirtioPciCapability {
                cap_next: 0,
                cap_len: Self::SIZE as u8,
                cfg_type: VIRTIO_PCI_CAP_PCI_CFG,
                bar,
                id: 0,
                offset,
                length,
            },
            pci_cfg_data: [0; 4],
        }
    }

    pub fn encode_le(self) -> [u8; Self::SIZE] {
        let mut out = [0u8; Self::SIZE];
        out[..VirtioPciCapability::SIZE].copy_from_slice(&self.cap.encode_le());
        out[16..20].copy_from_slice(&self.pci_cfg_data);
        out
    }

    pub fn decode_le(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < Self::SIZE {
            return None;
        }
        let cap = VirtioPciCapability::decode_le(bytes)?;
        if cap.cfg_type != VIRTIO_PCI_CAP_PCI_CFG || cap.cap_len < Self::SIZE as u8 {
            return None;
        }
        let mut pci_cfg_data = [0u8; 4];
        pci_cfg_data.copy_from_slice(&bytes[16..20]);
        Some(Self { cap, pci_cfg_data })
    }

    pub fn access_length(&self) -> Option<usize> {
        match self.cap.length {
            1 | 2 | 4 => Some(self.cap.length as usize),
            _ => None,
        }
    }

    pub fn set_access(&mut self, bar: u8, length: u32, offset: u32) -> bool {
        if !matches!(length, 1 | 2 | 4) || !offset.is_multiple_of(length) {
            return false;
        }
        self.cap.bar = bar;
        self.cap.length = length;
        self.cap.offset = offset;
        true
    }

    pub fn set_data(&mut self, data: &[u8]) -> bool {
        let Some(length) = self.access_length() else {
            return false;
        };
        if data.len() != length {
            return false;
        }
        self.pci_cfg_data[..length].copy_from_slice(data);
        true
    }

    pub fn data(&self) -> Option<&[u8]> {
        self.access_length()
            .map(|length| &self.pci_cfg_data[..length])
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PciConfigSpace {
    pub const SIZE: usize = 4096;
    pub const CAPABILITY_POINTER: usize = 0x34;
    pub const FIRST_CAPABILITY: u8 = 0x50;

    bytes: [u8; Self::SIZE],
}

impl PciConfigSpace {
    pub fn new(vendor_id: u16, device_id: u16) -> Self {
        let mut bytes = [0u8; Self::SIZE];
        bytes[0..2].copy_from_slice(&vendor_id.to_le_bytes());
        bytes[2..4].copy_from_slice(&device_id.to_le_bytes());
        bytes[0x06..0x08].copy_from_slice(&0x0010u16.to_le_bytes());
        bytes[0x08] = 0x00;
        bytes[0x09] = 0x00;
        bytes[0x0a] = 0x02;
        bytes[0x0b] = 0xFF;
        bytes[Self::CAPABILITY_POINTER] = Self::FIRST_CAPABILITY;
        Self { bytes }
    }

    pub fn read(&self, offset: usize, width: usize) -> Option<u64> {
        if !matches!(width, 1 | 2 | 4 | 8) {
            return None;
        }
        let end = offset.checked_add(width)?;
        if end > self.bytes.len() {
            return None;
        }
        let mut tmp = [0u8; 8];
        tmp[..width].copy_from_slice(&self.bytes[offset..end]);
        Some(u64::from_le_bytes(tmp))
    }

    pub fn write(&mut self, offset: usize, width: usize, value: u64) -> bool {
        if !matches!(width, 1 | 2 | 4 | 8) {
            return false;
        }
        let Some(end) = offset.checked_add(width) else {
            return false;
        };
        if end > self.bytes.len() {
            return false;
        }
        self.bytes[offset..end].copy_from_slice(&value.to_le_bytes()[..width]);
        true
    }

    pub fn bar(&self, index: u8) -> Option<u64> {
        if index >= 6 {
            return None;
        }
        let offset = 0x10 + usize::from(index) * 4;
        self.read(offset, 4)
    }

    pub fn set_bar_memory(&mut self, index: u8, base: u64) -> bool {
        if index >= 6 || base > u64::from(u32::MAX) {
            return false;
        }
        let offset = 0x10 + usize::from(index) * 4;
        self.write(offset, 4, base)
    }

    pub fn capability_pointer(&self) -> u8 {
        self.bytes[Self::CAPABILITY_POINTER]
    }

    pub fn bytes(&self) -> &[u8; Self::SIZE] {
        &self.bytes
    }

    pub fn install_capabilities(
        &mut self,
        common: &[VirtioPciCapability],
        notify: &[VirtioPciNotifyCapability],
        shared: &[VirtioPciCap64],
    ) -> Result<(), &'static str> {
        self.install_capabilities_with_pci_cfg(common, notify, shared, None)
    }

    pub fn install_capabilities_with_pci_cfg(
        &mut self,
        common: &[VirtioPciCapability],
        notify: &[VirtioPciNotifyCapability],
        shared: &[VirtioPciCap64],
        pci_cfg: Option<VirtioPciCfgCapability>,
    ) -> Result<(), &'static str> {
        let mut encoded = Vec::<Vec<u8>>::new();
        encoded.extend(common.iter().map(|cap| cap.encode_le().to_vec()));
        encoded.extend(notify.iter().map(|cap| cap.encode_le().to_vec()));
        encoded.extend(shared.iter().map(|cap| cap.encode_le().to_vec()));
        if let Some(cap) = pci_cfg {
            encoded.push(cap.encode_le().to_vec());
        }

        if encoded.is_empty() {
            return Err("a VirtIO PCI device must expose at least one capability");
        }

        let mut cursor = usize::from(Self::FIRST_CAPABILITY);
        let mut locations = Vec::with_capacity(encoded.len());
        for data in &encoded {
            cursor = (cursor + 3) & !3;
            let end = cursor
                .checked_add(data.len())
                .ok_or("capability space overflow")?;
            if end > Self::SIZE || cursor > u8::MAX as usize {
                return Err("capability list does not fit in PCI configuration space");
            }
            locations.push((cursor, data.len()));
            cursor = end;
        }

        self.bytes[Self::CAPABILITY_POINTER] = locations[0].0 as u8;
        for (index, ((offset, len), data)) in locations.iter().zip(encoded.iter()).enumerate() {
            let next = locations
                .get(index + 1)
                .map(|(next, _)| *next as u8)
                .unwrap_or(0);
            let mut data = data.clone();
            data[1] = next;
            data[2] = u8::try_from(*len).map_err(|_| "capability too large")?;
            self.bytes[*offset..*offset + *len].copy_from_slice(&data);
        }
        Ok(())
    }

    pub fn validate_bar(&self, bar: PciBar) -> bool {
        self.bar(bar.index).is_some_and(|base| base == bar.base)
    }
}
