pub mod common;

use std::collections::HashMap;

use crate::virtio_gpu::device::{DeviceError, VirtioGpuDevice};
use crate::virtio_gpu::features::GpuFeatures;

use common::CommonConfig;

pub const PCI_VENDOR_ID_VIRTIO: u16 = 0x1af4;
pub const PCI_DEVICE_ID_GPU: u16 = 0x1050;

pub const PCI_CAP_ID_VENDOR: u8 = 0x09;

pub const VIRTIO_PCI_CAP_COMMON_CFG: u8 = 1;
pub const VIRTIO_PCI_CAP_NOTIFY_CFG: u8 = 2;
pub const VIRTIO_PCI_CAP_ISR_CFG: u8 = 3;
pub const VIRTIO_PCI_CAP_DEVICE_CFG: u8 = 4;
pub const VIRTIO_PCI_CAP_PCI_CFG: u8 = 5;
pub const VIRTIO_PCI_CAP_SHARED_MEMORY_CFG: u8 = 8;

const NUM_QUEUES: u16 = 2;
const MAX_QUEUE_SIZE: u16 = 32768;

/// Standard VirtIO PCI capability.
///
/// Layout follows `struct virtio_pci_cap` from the modern VirtIO PCI
/// transport. The `id` field is part of the current structure and is used
/// to identify multiple capabilities of a given type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VirtioPciCapability {
    pub cap_next: u8,
    pub cap_len: u8,
    pub cfg_type: u8,
    pub bar: u8,
    pub id: u8,
    pub offset: u32,
    pub length: u32,
}

impl VirtioPciCapability {
    pub const SIZE: usize = 16;

    pub const fn new(cfg_type: u8, bar: u8, offset: u32, length: u32) -> Self {
        Self::new_with_id(cfg_type, bar, 0, offset, length)
    }

    pub const fn new_with_id(cfg_type: u8, bar: u8, id: u8, offset: u32, length: u32) -> Self {
        Self {
            cap_next: 0,
            cap_len: Self::SIZE as u8,
            cfg_type,
            bar,
            id,
            offset,
            length,
        }
    }

    pub fn encode_le(self) -> [u8; Self::SIZE] {
        let mut out = [0u8; Self::SIZE];
        out[0] = PCI_CAP_ID_VENDOR;
        out[1] = self.cap_next;
        out[2] = self.cap_len;
        out[3] = self.cfg_type;
        out[4] = self.bar;
        out[5] = self.id;
        out[8..12].copy_from_slice(&self.offset.to_le_bytes());
        out[12..16].copy_from_slice(&self.length.to_le_bytes());
        out
    }

    pub fn decode_le(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < Self::SIZE || bytes[0] != PCI_CAP_ID_VENDOR {
            return None;
        }
        if bytes[2] < Self::SIZE as u8 {
            return None;
        }
        Some(Self {
            cap_next: bytes[1],
            cap_len: bytes[2],
            cfg_type: bytes[3],
            bar: bytes[4],
            id: bytes[5],
            offset: u32::from_le_bytes(bytes[8..12].try_into().ok()?),
            length: u32::from_le_bytes(bytes[12..16].try_into().ok()?),
        })
    }
}

/// VirtIO PCI notification capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VirtioPciNotifyCapability {
    pub cap: VirtioPciCapability,
    pub notify_off_multiplier: u32,
}

impl VirtioPciNotifyCapability {
    pub const SIZE: usize = VirtioPciCapability::SIZE + 4;

    pub const fn new(bar: u8, offset: u32, length: u32, notify_off_multiplier: u32) -> Self {
        Self {
            cap: VirtioPciCapability {
                cap_next: 0,
                cap_len: Self::SIZE as u8,
                cfg_type: VIRTIO_PCI_CAP_NOTIFY_CFG,
                bar,
                id: 0,
                offset,
                length,
            },
            notify_off_multiplier,
        }
    }

    pub fn encode_le(self) -> [u8; Self::SIZE] {
        let mut out = [0u8; Self::SIZE];
        out[..VirtioPciCapability::SIZE].copy_from_slice(&self.cap.encode_le());
        out[16..20].copy_from_slice(&self.notify_off_multiplier.to_le_bytes());
        out
    }

    pub fn decode_le(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < Self::SIZE {
            return None;
        }
        let cap = VirtioPciCapability::decode_le(bytes)?;
        if cap.cfg_type != VIRTIO_PCI_CAP_NOTIFY_CFG || cap.cap_len < Self::SIZE as u8 {
            return None;
        }
        Some(Self {
            cap,
            notify_off_multiplier: u32::from_le_bytes(bytes[16..20].try_into().ok()?),
        })
    }
}

/// 64-bit VirtIO PCI capability used by shared-memory regions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VirtioPciCap64 {
    pub cap: VirtioPciCapability,
    pub offset_hi: u32,
    pub length_hi: u32,
}

impl VirtioPciCap64 {
    pub const SIZE: usize = VirtioPciCapability::SIZE + 8;

    pub const fn new(bar: u8, id: u8, offset: u64, length: u64) -> Self {
        Self {
            cap: VirtioPciCapability {
                cap_next: 0,
                cap_len: Self::SIZE as u8,
                cfg_type: VIRTIO_PCI_CAP_SHARED_MEMORY_CFG,
                bar,
                id,
                offset: offset as u32,
                length: length as u32,
            },
            offset_hi: (offset >> 32) as u32,
            length_hi: (length >> 32) as u32,
        }
    }

    pub fn offset(&self) -> u64 {
        (u64::from(self.offset_hi) << 32) | u64::from(self.cap.offset)
    }

    pub fn length(&self) -> u64 {
        (u64::from(self.length_hi) << 32) | u64::from(self.cap.length)
    }

    pub fn encode_le(self) -> [u8; Self::SIZE] {
        let mut out = [0u8; Self::SIZE];
        out[..VirtioPciCapability::SIZE].copy_from_slice(&self.cap.encode_le());
        out[16..20].copy_from_slice(&self.offset_hi.to_le_bytes());
        out[20..24].copy_from_slice(&self.length_hi.to_le_bytes());
        out
    }

    pub fn decode_le(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < Self::SIZE {
            return None;
        }
        let cap = VirtioPciCapability::decode_le(bytes)?;
        if cap.cfg_type != VIRTIO_PCI_CAP_SHARED_MEMORY_CFG || cap.cap_len < Self::SIZE as u8 {
            return None;
        }
        Some(Self {
            cap,
            offset_hi: u32::from_le_bytes(bytes[16..20].try_into().ok()?),
            length_hi: u32::from_le_bytes(bytes[20..24].try_into().ok()?),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PciBar {
    pub index: u8,
    pub base: u64,
    pub size: u64,
}

impl PciBar {
    pub const fn new(index: u8, base: u64, size: u64) -> Self {
        Self { index, base, size }
    }

    pub fn contains(&self, offset: u64, length: u64) -> bool {
        offset.checked_add(length).is_some_and(|end| end <= self.size)
    }

    pub fn contains_abs(&self, address: u64, length: u64) -> bool {
        address >= self.base && self.contains(address - self.base, length)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SharedMemoryRegion {
    pub id: u32,
    pub bar: u8,
    pub offset: u64,
    pub length: u64,
    pub base: u64,
}

impl SharedMemoryRegion {
    pub fn end(&self) -> Option<u64> {
        self.base.checked_add(self.length)
    }

    pub fn contains(&self, address: u64, length: u64) -> bool {
        self.base <= address
            && address
                .checked_sub(self.base)
                .and_then(|off| off.checked_add(length))
                .is_some_and(|end| end <= self.length)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct QueueState {
    pub size: u16,
    pub enabled: bool,
    pub desc_addr: u64,
    pub driver_addr: u64,
    pub device_addr: u64,
    pub notify_off: u16,
}

#[derive(Debug, PartialEq, Eq)]
pub enum PciTransportError {
    InvalidBar,
    CapabilityOutOfBounds,
    InvalidCapability,
    UnsupportedAccess,
    QueueNotSelected,
    InvalidRegister,
    InvalidValue,
    InvalidQueueIndex,
    SharedMemoryNotFound,
    SharedMemoryIdExists,
    Device(DeviceError),
}

impl From<DeviceError> for PciTransportError {
    fn from(value: DeviceError) -> Self {
        Self::Device(value)
    }
}

pub struct VirtioPciTransport {
    pub device: VirtioGpuDevice,
    pub common_config: CommonConfig,
    bars: [Option<PciBar>; 6],
    capabilities: Vec<VirtioPciCapability>,
    shared_memory_capabilities: Vec<VirtioPciCap64>,
    shared_memory_regions: HashMap<u32, SharedMemoryRegion>,
    shared_memory_bytes: HashMap<u32, Vec<u8>>,
    queue_states: [QueueState; NUM_QUEUES as usize],
    device_config: Vec<u8>,
    selected_queue: Option<u16>,
    notify_off_multiplier: u32,
    isr_status: u8,
}

impl VirtioPciTransport {
    pub fn new(device: VirtioGpuDevice) -> Self {
        let config_generation = device.config_generation();
        let queue_state = QueueState {
            size: MAX_QUEUE_SIZE,
            enabled: false,
            ..QueueState::default()
        };
        Self {
            device,
            common_config: CommonConfig::new(NUM_QUEUES, config_generation),
            bars: [None; 6],
            capabilities: Vec::new(),
            shared_memory_capabilities: Vec::new(),
            shared_memory_regions: HashMap::new(),
            shared_memory_bytes: HashMap::new(),
            queue_states: [queue_state; NUM_QUEUES as usize],
            device_config: Vec::new(),
            selected_queue: None,
            notify_off_multiplier: 0,
            isr_status: 0,
        }
    }

    pub fn common_config(&self) -> &CommonConfig {
        &self.common_config
    }

    pub fn common_config_mut(&mut self) -> &mut CommonConfig {
        &mut self.common_config
    }

    pub fn add_bar(&mut self, bar: PciBar) -> Result<(), PciTransportError> {
        if bar.index >= 6 || bar.size == 0 {
            return Err(PciTransportError::InvalidBar);
        }
        self.bars[bar.index as usize] = Some(bar);
        Ok(())
    }

    pub fn bar(&self, index: u8) -> Option<PciBar> {
        self.bars.get(index as usize).copied().flatten()
    }

    pub fn add_capability(&mut self, capability: VirtioPciCapability) -> Result<(), PciTransportError> {
        let bar = self.bar(capability.bar).ok_or(PciTransportError::InvalidBar)?;
        if capability.cap_len < VirtioPciCapability::SIZE as u8 || capability.cfg_type == 0 {
            return Err(PciTransportError::InvalidCapability);
        }
        if !bar.contains(capability.offset as u64, capability.length as u64) {
            return Err(PciTransportError::CapabilityOutOfBounds);
        }
        if matches!(capability.cfg_type, VIRTIO_PCI_CAP_COMMON_CFG | VIRTIO_PCI_CAP_DEVICE_CFG)
            && !capability.offset.is_multiple_of(4)
        {
            return Err(PciTransportError::InvalidCapability);
        }
        if capability.cfg_type == VIRTIO_PCI_CAP_NOTIFY_CFG
            && (!capability.offset.is_multiple_of(2) || capability.length < 2)
        {
            return Err(PciTransportError::InvalidCapability);
        }
        if capability.cfg_type == VIRTIO_PCI_CAP_ISR_CFG && capability.length < 1 {
            return Err(PciTransportError::InvalidCapability);
        }
        if capability.cfg_type == VIRTIO_PCI_CAP_DEVICE_CFG {
            self.device_config.resize(capability.length as usize, 0);
        }
        self.capabilities.push(capability);
        Ok(())
    }

    pub fn add_notify_capability(&mut self, capability: VirtioPciNotifyCapability) -> Result<(), PciTransportError> {
        self.add_capability(capability.cap)?;
        if let Some(last) = self.capabilities.last_mut() {
            last.cap_len = VirtioPciNotifyCapability::SIZE as u8;
        }
        self.notify_off_multiplier = capability.notify_off_multiplier;
        Ok(())
    }

    pub fn add_shared_memory_region(
        &mut self,
        bar: u8,
        id: u32,
        offset: u64,
        length: u64,
    ) -> Result<(), PciTransportError> {
        if self.shared_memory_regions.contains_key(&id) {
            return Err(PciTransportError::SharedMemoryIdExists);
        }
        let pci_bar = self.bar(bar).ok_or(PciTransportError::InvalidBar)?;
        if length == 0 || !pci_bar.contains(offset, length) {
            return Err(PciTransportError::CapabilityOutOfBounds);
        }
        let base = pci_bar
            .base
            .checked_add(offset)
            .ok_or(PciTransportError::InvalidValue)?;
        let region = SharedMemoryRegion {
            id,
            bar,
            offset,
            length,
            base,
        };
        let bytes_len = usize::try_from(length).map_err(|_| PciTransportError::InvalidValue)?;
        self.shared_memory_regions.insert(id, region);
        self.shared_memory_bytes.insert(id, vec![0; bytes_len]);
        self.shared_memory_capabilities
            .push(VirtioPciCap64::new(bar, u8::try_from(id).map_err(|_| PciTransportError::InvalidValue)?, offset, length));
        if self.common_config.shm_sel == id {
            self.common_config.set_shared_memory_region(length, base);
        }
        Ok(())
    }

    pub fn shared_memory_region(&self, id: u32) -> Option<SharedMemoryRegion> {
        self.shared_memory_regions.get(&id).copied()
    }

    pub fn shared_memory_region_bytes(&self, id: u32) -> Result<&[u8], PciTransportError> {
        self.shared_memory_bytes
            .get(&id)
            .map(Vec::as_slice)
            .ok_or(PciTransportError::SharedMemoryNotFound)
    }

    pub fn shared_memory_region_bytes_mut(&mut self, id: u32) -> Result<&mut [u8], PciTransportError> {
        self.shared_memory_bytes
            .get_mut(&id)
            .map(Vec::as_mut_slice)
            .ok_or(PciTransportError::SharedMemoryNotFound)
    }

    pub fn shared_memory_capability(&self, id: u32) -> Option<VirtioPciCap64> {
        self.shared_memory_capabilities
            .iter()
            .copied()
            .find(|cap| u32::from(cap.cap.id) == id)
    }

    pub fn capabilities(&self) -> &[VirtioPciCapability] {
        &self.capabilities
    }

    pub fn shared_memory_capabilities(&self) -> &[VirtioPciCap64] {
        &self.shared_memory_capabilities
    }

    pub fn capability(&self, cfg_type: u8) -> Option<VirtioPciCapability> {
        self.capabilities.iter().copied().find(|cap| cap.cfg_type == cfg_type)
    }

    pub fn notify_off_multiplier(&self) -> u32 {
        self.notify_off_multiplier
    }

    pub fn isr_status(&self) -> u8 {
        self.isr_status
    }

    pub fn read_isr(&mut self) -> u8 {
        let status = self.isr_status;
        self.isr_status = 0;
        status
    }

    pub fn set_isr_status(&mut self, status: u8) {
        self.isr_status |= status;
    }

    pub fn select_queue(&mut self, queue_index: u16) -> Result<(), PciTransportError> {
        if queue_index >= self.common_config.num_queues {
            return Err(PciTransportError::InvalidQueueIndex);
        }
        self.common_config.queue_select = queue_index;
        self.selected_queue = Some(queue_index);
        let state = self.queue_states[queue_index as usize];
        self.common_config.queue_size = state.size;
        self.common_config.queue_msix_vector = CommonConfig::MSIX_VECTOR_NONE;
        self.common_config.queue_enable = u16::from(state.enabled);
        self.common_config.queue_notify_off = state.notify_off;
        self.common_config.queue_desc = state.desc_addr;
        self.common_config.queue_driver = state.driver_addr;
        self.common_config.queue_device = state.device_addr;
        Ok(())
    }

    pub fn selected_queue(&self) -> Option<u16> {
        self.selected_queue
    }

    pub fn offered_features(&self) -> GpuFeatures {
        self.device.device_features()
    }

    fn selected_queue_index(&self) -> Result<usize, PciTransportError> {
        let index = self.selected_queue.ok_or(PciTransportError::QueueNotSelected)?;
        if index >= self.common_config.num_queues {
            return Err(PciTransportError::InvalidQueueIndex);
        }
        Ok(index as usize)
    }

    fn select_shared_memory(&mut self, id: u32) {
        self.common_config.shm_sel = id;
        if let Some(region) = self.shared_memory_regions.get(&id).copied() {
            self.common_config.set_shared_memory_region(region.length, region.base);
        } else {
            self.common_config.set_no_shared_memory_region();
        }
    }

    pub fn read_common(&self, offset: u64, width: usize) -> Result<u64, PciTransportError> {
        match (offset, width) {
            (CommonConfig::DEVICE_FEATURE_SELECT, 4) => Ok(self.common_config.device_feature_select as u64),
            (CommonConfig::DEVICE_FEATURE, 4) => {
                let select = self.common_config.device_feature_select;
                Ok(if select <= 1 { (self.device.device_features().bits() >> (select * 32)) & 0xffff_ffff } else { 0 })
            }
            (CommonConfig::DRIVER_FEATURE_SELECT, 4) => Ok(self.common_config.driver_feature_select as u64),
            (CommonConfig::DRIVER_FEATURE, 4) => {
                let select = self.common_config.driver_feature_select;
                Ok(if select <= 1 { (self.device.driver_features().bits() >> (select * 32)) & 0xffff_ffff } else { 0 })
            }
            (CommonConfig::CONFIG_MSIX_VECTOR, 2) => Ok(self.common_config.config_msix_vector as u64),
            (CommonConfig::NUM_QUEUES, 2) => Ok(self.common_config.num_queues as u64),
            (CommonConfig::DEVICE_STATUS, 1) => Ok(self.device.status().bits() as u64),
            (CommonConfig::CONFIG_GENERATION, 1) => Ok(self.device.config_generation() as u64),
            (CommonConfig::QUEUE_SELECT, 2) => Ok(self.common_config.queue_select as u64),
            (CommonConfig::QUEUE_SIZE, 2) => Ok(self.common_config.queue_size as u64),
            (CommonConfig::QUEUE_MSIX_VECTOR, 2) => Ok(self.common_config.queue_msix_vector as u64),
            (CommonConfig::QUEUE_ENABLE, 2) => Ok(self.common_config.queue_enable as u64),
            (CommonConfig::QUEUE_NOTIFY_OFF, 2) => Ok(self.common_config.queue_notify_off as u64),
            (CommonConfig::QUEUE_DESC, 8) => Ok(self.common_config.queue_desc),
            (CommonConfig::QUEUE_DRIVER, 8) => Ok(self.common_config.queue_driver),
            (CommonConfig::QUEUE_DEVICE, 8) => Ok(self.common_config.queue_device),
            (CommonConfig::QUEUE_NOTIFY_DATA, 2) => Ok(self.common_config.queue_notify_data as u64),
            (CommonConfig::SHM_SEL, 4) => Ok(self.common_config.shm_sel as u64),
            (CommonConfig::SHM_LEN_LOW, 4) => Ok(self.common_config.shm_len_low as u64),
            (CommonConfig::SHM_LEN_HIGH, 4) => Ok(self.common_config.shm_len_high as u64),
            (CommonConfig::SHM_BASE_LOW, 4) => Ok(self.common_config.shm_base_low as u64),
            (CommonConfig::SHM_BASE_HIGH, 4) => Ok(self.common_config.shm_base_high as u64),
            (CommonConfig::QUEUE_RESET, 2) => Ok(self.common_config.queue_reset as u64),
            _ => Err(PciTransportError::UnsupportedAccess),
        }
    }

    pub fn write_common(&mut self, offset: u64, width: usize, value: u64) -> Result<(), PciTransportError> {
        match (offset, width) {
            (CommonConfig::DEVICE_FEATURE_SELECT, 4) => self.common_config.device_feature_select = value as u32,
            (CommonConfig::DRIVER_FEATURE_SELECT, 4) => self.common_config.driver_feature_select = value as u32,
            (CommonConfig::DRIVER_FEATURE, 4) => {
                let select = self.common_config.driver_feature_select;
                if select <= 1 {
                    let current = self.device.driver_features().bits();
                    let mask = 0xffff_ffffu64 << (select * 32);
                    let features = (current & !mask) | ((value & 0xffff_ffff) << (select * 32));
                    self.device.set_driver_features(features)?;
                }
            }
            (CommonConfig::CONFIG_MSIX_VECTOR, 2) => self.common_config.config_msix_vector = value as u16,
            (CommonConfig::DEVICE_STATUS, 1) => {
                self.device.set_status(value as u8)?;
                self.common_config.device_status = self.device.status().bits();
                if self.device.status().is_empty() {
                    self.reset_transport_state();
                }
            }
            (CommonConfig::QUEUE_SELECT, 2) => self.select_queue(value as u16)?,
            (CommonConfig::QUEUE_SIZE, 2) => {
                let index = self.selected_queue_index()?;
                let queue_size = value as u16;
                if queue_size == 0 || queue_size > MAX_QUEUE_SIZE || !queue_size.is_power_of_two() {
                    return Err(PciTransportError::InvalidValue);
                }
                self.queue_states[index].size = queue_size;
                self.common_config.queue_size = queue_size;
            }
            (CommonConfig::QUEUE_MSIX_VECTOR, 2) => {
                self.selected_queue_index()?;
                self.common_config.queue_msix_vector = value as u16;
            }
            (CommonConfig::QUEUE_ENABLE, 2) => {
                let index = self.selected_queue_index()?;
                if value != 1 || self.common_config.queue_size == 0 {
                    return Err(PciTransportError::InvalidValue);
                }
                if !self.device.queue_ready(self.common_config.queue_select)? {
                    return Err(PciTransportError::QueueNotSelected);
                }
                self.queue_states[index].enabled = true;
                self.common_config.queue_enable = 1;
            }
            (CommonConfig::QUEUE_DESC, 8) => {
                let index = self.selected_queue_index()?;
                self.queue_states[index].desc_addr = value;
                self.common_config.queue_desc = value;
            }
            (CommonConfig::QUEUE_DRIVER, 8) => {
                let index = self.selected_queue_index()?;
                self.queue_states[index].driver_addr = value;
                self.common_config.queue_driver = value;
            }
            (CommonConfig::QUEUE_DEVICE, 8) => {
                let index = self.selected_queue_index()?;
                self.queue_states[index].device_addr = value;
                self.common_config.queue_device = value;
            }
            (CommonConfig::QUEUE_NOTIFY_DATA, 2) => {
                self.selected_queue_index()?;
                self.common_config.queue_notify_data = value as u16;
            }
            (CommonConfig::SHM_SEL, 4) => self.select_shared_memory(value as u32),
            (CommonConfig::QUEUE_RESET, 2) => {
                if value != 1 {
                    return Err(PciTransportError::InvalidValue);
                }
                let index = self.selected_queue_index()?;
                self.queue_states[index] = QueueState::default();
                self.common_config.reset_queue();
            }
            (CommonConfig::DEVICE_FEATURE, _)
            | (CommonConfig::NUM_QUEUES, _)
            | (CommonConfig::CONFIG_GENERATION, _)
            | (CommonConfig::QUEUE_NOTIFY_OFF, _)
            | (CommonConfig::SHM_LEN_LOW, _)
            | (CommonConfig::SHM_LEN_HIGH, _)
            | (CommonConfig::SHM_BASE_LOW, _)
            | (CommonConfig::SHM_BASE_HIGH, _) => return Err(PciTransportError::UnsupportedAccess),
            _ => return Err(PciTransportError::UnsupportedAccess),
        }
        Ok(())
    }

    fn reset_transport_state(&mut self) {
        self.common_config = CommonConfig::new(NUM_QUEUES, self.device.config_generation());
        self.selected_queue = None;
        let queue_state = QueueState {
            size: MAX_QUEUE_SIZE,
            ..QueueState::default()
        };
        self.queue_states = [queue_state; NUM_QUEUES as usize];
        self.device_config.fill(0);
        self.isr_status = 0;
    }

    pub fn initialize_default_capabilities(&mut self) -> Result<(), PciTransportError> {
        self.add_bar(PciBar::new(0, 0x1000_0000, 0x1000))?;
        self.add_bar(PciBar::new(2, 0x2000_0000, 0x0400_0000))?;

        self.add_capability(VirtioPciCapability::new(
            VIRTIO_PCI_CAP_COMMON_CFG,
            0,
            0x000,
            CommonConfig::SIZE as u32,
        ))?;
        self.add_notify_capability(VirtioPciNotifyCapability::new(0, 0x100, 0x100, 4))?;
        self.add_capability(VirtioPciCapability::new(VIRTIO_PCI_CAP_ISR_CFG, 0, 0x200, 1))?;
        self.add_capability(VirtioPciCapability::new(VIRTIO_PCI_CAP_DEVICE_CFG, 0, 0x300, 0x100))?;
        self.add_shared_memory_region(2, 1, 0, 0x0400_0000)?;
        Ok(())
    }

    pub fn read_device_config(&self, offset: u64, width: usize) -> Result<u64, PciTransportError> {
        if !matches!(width, 1 | 2 | 4 | 8) {
            return Err(PciTransportError::UnsupportedAccess);
        }
        let end = offset.checked_add(width as u64).ok_or(PciTransportError::InvalidValue)?;
        if end > self.device_config.len() as u64 {
            return Err(PciTransportError::InvalidValue);
        }
        let start = offset as usize;
        let mut bytes = [0u8; 8];
        bytes[..width].copy_from_slice(&self.device_config[start..start + width]);
        Ok(u64::from_le_bytes(bytes))
    }

    pub fn write_device_config(&mut self, offset: u64, width: usize, value: u64) -> Result<(), PciTransportError> {
        if !matches!(width, 1 | 2 | 4 | 8) {
            return Err(PciTransportError::UnsupportedAccess);
        }
        let end = offset.checked_add(width as u64).ok_or(PciTransportError::InvalidValue)?;
        if end > self.device_config.len() as u64 {
            return Err(PciTransportError::InvalidValue);
        }
        let start = offset as usize;
        let bytes = value.to_le_bytes();
        self.device_config[start..start + width].copy_from_slice(&bytes[..width]);
        Ok(())
    }

    pub fn queue_state(&self, queue_index: u16) -> Result<QueueState, PciTransportError> {
        if queue_index >= NUM_QUEUES {
            return Err(PciTransportError::InvalidQueueIndex);
        }
        Ok(self.queue_states[queue_index as usize])
    }

    pub fn queue_notify_address(&self, queue_index: u16) -> Result<u64, PciTransportError> {
        let capability = self.capability(VIRTIO_PCI_CAP_NOTIFY_CFG).ok_or(PciTransportError::InvalidCapability)?;
        let queue = self.queue_state(queue_index)?;
        let offset = u64::from(queue.notify_off)
            .checked_mul(u64::from(self.notify_off_multiplier))
            .ok_or(PciTransportError::InvalidValue)?;
        u64::from(capability.offset).checked_add(offset).ok_or(PciTransportError::InvalidValue)
    }
}

impl Default for VirtioPciTransport {
    fn default() -> Self {
        Self::new(VirtioGpuDevice::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::virtio_gpu::device::DeviceStatus;

    #[test]
    fn virtio_gpu_pci_ids_are_correct() {
        assert_eq!(PCI_VENDOR_ID_VIRTIO, 0x1af4);
        assert_eq!(PCI_DEVICE_ID_GPU, 0x1050);
    }

    #[test]
    fn capability_round_trip_includes_id() {
        let capability = VirtioPciCapability::new_with_id(VIRTIO_PCI_CAP_SHARED_MEMORY_CFG, 2, 7, 0x1000, 0x2000);
        let decoded = VirtioPciCapability::decode_le(&capability.encode_le()).unwrap();
        assert_eq!(decoded, capability);
    }

    #[test]
    fn cap64_round_trip() {
        let capability = VirtioPciCap64::new(2, 7, 0x1_0000_2000, 0x2_0000_3000);
        let decoded = VirtioPciCap64::decode_le(&capability.encode_le()).unwrap();
        assert_eq!(decoded, capability);
        assert_eq!(decoded.offset(), 0x1_0000_2000);
        assert_eq!(decoded.length(), 0x2_0000_3000);
    }

    #[test]
    fn notify_capability_round_trip() {
        let capability = VirtioPciNotifyCapability::new(0, 0x100, 0x100, 4);
        let decoded = VirtioPciNotifyCapability::decode_le(&capability.encode_le()).unwrap();
        assert_eq!(decoded, capability);
    }

    #[test]
    fn bar_contains_region() {
        let bar = PciBar::new(0, 0x1000, 0x1000);
        assert!(bar.contains(0, 0x100));
        assert!(bar.contains(0xF00, 0x100));
        assert!(!bar.contains(0xF01, 0x100));
    }

    #[test]
    fn capability_outside_bar_is_rejected() {
        let mut transport = VirtioPciTransport::default();
        transport.add_bar(PciBar::new(0, 0x1000, 0x1000)).unwrap();
        let capability = VirtioPciCapability::new(VIRTIO_PCI_CAP_COMMON_CFG, 0, 0xF01, 0x100);
        assert_eq!(transport.add_capability(capability), Err(PciTransportError::CapabilityOutOfBounds));
    }

    #[test]
    fn feature_select_reads_words() {
        let mut transport = VirtioPciTransport::default();
        let features = transport.device.device_features().bits();
        assert_eq!(transport.read_common(CommonConfig::DEVICE_FEATURE, 4).unwrap(), features & 0xffff_ffff);
        transport.write_common(CommonConfig::DEVICE_FEATURE_SELECT, 4, 1).unwrap();
        assert_eq!(transport.read_common(CommonConfig::DEVICE_FEATURE, 4).unwrap(), features >> 32);
    }

    #[test]
    fn driver_feature_negotiation_works() {
        let mut transport = VirtioPciTransport::default();
        let features = transport.read_common(CommonConfig::DEVICE_FEATURE, 4).unwrap();
        transport.write_common(CommonConfig::DRIVER_FEATURE, 4, features).unwrap();
        assert_eq!(transport.device.driver_features(), GpuFeatures::from_bits_truncate(features));
    }

    #[test]
    fn status_write_works() {
        let mut transport = VirtioPciTransport::default();
        transport.write_common(CommonConfig::DEVICE_STATUS, 1, DeviceStatus::ACKNOWLEDGE.bits() as u64).unwrap();
        assert!(transport.device.status().contains(DeviceStatus::ACKNOWLEDGE));
    }

    #[test]
    fn queue_selection_works() {
        let mut transport = VirtioPciTransport::default();
        transport.select_queue(0).unwrap();
        assert_eq!(transport.selected_queue(), Some(0));
        assert_eq!(transport.read_common(CommonConfig::QUEUE_SELECT, 2).unwrap(), 0);
    }

    #[test]
    fn default_capabilities_include_shared_memory() {
        let mut transport = VirtioPciTransport::default();
        transport.initialize_default_capabilities().unwrap();
        assert!(transport.capability(VIRTIO_PCI_CAP_COMMON_CFG).is_some());
        assert!(transport.capability(VIRTIO_PCI_CAP_DEVICE_CFG).is_some());
        assert!(transport.capability(VIRTIO_PCI_CAP_NOTIFY_CFG).is_some());
        assert_eq!(transport.shared_memory_capabilities().len(), 1);
        let region = transport.shared_memory_region(1).unwrap();
        assert_eq!(region.base, 0x2000_0000);
        assert_eq!(region.length, 0x0400_0000);
    }

    #[test]
    fn shm_select_updates_common_registers() {
        let mut transport = VirtioPciTransport::default();
        transport.initialize_default_capabilities().unwrap();
        transport.write_common(CommonConfig::SHM_SEL, 4, 1).unwrap();
        assert_eq!(transport.read_common(CommonConfig::SHM_LEN_LOW, 4).unwrap(), 0x0400_0000);
        assert_eq!(transport.read_common(CommonConfig::SHM_LEN_HIGH, 4).unwrap(), 0);
        assert_eq!(transport.read_common(CommonConfig::SHM_BASE_LOW, 4).unwrap(), 0x2000_0000);
        assert_eq!(transport.read_common(CommonConfig::SHM_BASE_HIGH, 4).unwrap(), 0);
    }

    #[test]
    fn invalid_shm_id_returns_minus_one_registers() {
        let mut transport = VirtioPciTransport::default();
        transport.initialize_default_capabilities().unwrap();
        transport.write_common(CommonConfig::SHM_SEL, 4, 999).unwrap();
        assert_eq!(transport.read_common(CommonConfig::SHM_LEN_LOW, 4).unwrap(), u32::MAX as u64);
        assert_eq!(transport.read_common(CommonConfig::SHM_BASE_LOW, 4).unwrap(), u32::MAX as u64);
    }

    #[test]
    fn shared_memory_bytes_are_real_storage() {
        let mut transport = VirtioPciTransport::default();
        transport.initialize_default_capabilities().unwrap();
        let bytes = transport.shared_memory_region_bytes_mut(1).unwrap();
        bytes[..4].copy_from_slice(&[1, 2, 3, 4]);
        assert_eq!(&transport.shared_memory_region_bytes(1).unwrap()[..4], &[1, 2, 3, 4]);
    }

    #[test]
    fn device_config_round_trip() {
        let mut transport = VirtioPciTransport::default();
        transport.initialize_default_capabilities().unwrap();
        transport.write_device_config(4, 4, 0xdead_beef).unwrap();
        assert_eq!(transport.read_device_config(4, 4).unwrap(), 0xdead_beef);
    }
}
