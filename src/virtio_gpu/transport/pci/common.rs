//! VirtIO PCI common configuration space.
//!
//! Layout follows the modern VirtIO PCI transport, including the shared-memory
//! selector/length/base registers and queue reset register.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct CommonConfig {
    pub device_feature_select: u32,
    pub driver_feature_select: u32,
    pub config_msix_vector: u16,
    pub num_queues: u16,
    pub device_status: u8,
    pub config_generation: u8,
    pub queue_select: u16,
    pub queue_size: u16,
    pub queue_msix_vector: u16,
    pub queue_enable: u16,
    pub queue_notify_off: u16,
    pub queue_desc: u64,
    pub queue_driver: u64,
    pub queue_device: u64,
    pub queue_notify_data: u16,
    pub shm_sel: u32,
    pub shm_len_low: u32,
    pub shm_len_high: u32,
    pub shm_base_low: u32,
    pub shm_base_high: u32,
    pub queue_reset: u16,
}

impl CommonConfig {
    pub const MSIX_VECTOR_NONE: u16 = u16::MAX;

    pub const DEVICE_FEATURE_SELECT: u64 = 0x00;
    pub const DEVICE_FEATURE: u64 = 0x04;
    pub const DRIVER_FEATURE_SELECT: u64 = 0x08;
    pub const DRIVER_FEATURE: u64 = 0x0c;
    pub const CONFIG_MSIX_VECTOR: u64 = 0x10;
    pub const NUM_QUEUES: u64 = 0x12;
    pub const DEVICE_STATUS: u64 = 0x14;
    pub const QUEUE_SELECT: u64 = 0x16;
    pub const QUEUE_SIZE: u64 = 0x18;
    pub const QUEUE_MSIX_VECTOR: u64 = 0x1a;
    pub const QUEUE_ENABLE: u64 = 0x1c;
    pub const QUEUE_NOTIFY_OFF: u64 = 0x1e;
    pub const QUEUE_DESC: u64 = 0x20;
    pub const QUEUE_DRIVER: u64 = 0x28;
    pub const QUEUE_DEVICE: u64 = 0x30;
    pub const QUEUE_NOTIFY_DATA: u64 = 0x38;
    pub const SHM_SEL: u64 = 0xac;
    pub const SHM_LEN_LOW: u64 = 0xb0;
    pub const SHM_LEN_HIGH: u64 = 0xb4;
    pub const SHM_BASE_LOW: u64 = 0xb8;
    pub const SHM_BASE_HIGH: u64 = 0xbc;
    pub const QUEUE_RESET: u64 = 0xc0;
    pub const CONFIG_GENERATION: u64 = 0xfc;
    pub const SIZE: u64 = 0x100;

    pub const fn new(num_queues: u16, config_generation: u8) -> Self {
        Self {
            device_feature_select: 0,
            driver_feature_select: 0,
            config_msix_vector: Self::MSIX_VECTOR_NONE,
            num_queues,
            device_status: 0,
            config_generation,
            queue_select: 0,
            queue_size: 0,
            queue_msix_vector: Self::MSIX_VECTOR_NONE,
            queue_enable: 0,
            queue_notify_off: 0,
            queue_desc: 0,
            queue_driver: 0,
            queue_device: 0,
            queue_notify_data: 0,
            shm_sel: 0,
            shm_len_low: u32::MAX,
            shm_len_high: u32::MAX,
            shm_base_low: u32::MAX,
            shm_base_high: u32::MAX,
            queue_reset: 0,
        }
    }

    pub fn reset_queue(&mut self) {
        self.queue_size = 0;
        self.queue_msix_vector = Self::MSIX_VECTOR_NONE;
        self.queue_enable = 0;
        self.queue_notify_off = 0;
        self.queue_desc = 0;
        self.queue_driver = 0;
        self.queue_device = 0;
        self.queue_notify_data = 0;
        self.queue_reset = 0;
    }

    pub fn set_shared_memory_region(&mut self, length: u64, base: u64) {
        let length = length.to_le_bytes();
        let base = base.to_le_bytes();
        self.shm_len_low = u32::from_le_bytes(length[..4].try_into().expect("slice size"));
        self.shm_len_high = u32::from_le_bytes(length[4..].try_into().expect("slice size"));
        self.shm_base_low = u32::from_le_bytes(base[..4].try_into().expect("slice size"));
        self.shm_base_high = u32::from_le_bytes(base[4..].try_into().expect("slice size"));
    }

    pub fn set_no_shared_memory_region(&mut self) {
        self.shm_len_low = u32::MAX;
        self.shm_len_high = u32::MAX;
        self.shm_base_low = u32::MAX;
        self.shm_base_high = u32::MAX;
    }

    pub fn read_u32(&self, offset: u64) -> Option<u32> {
        match offset {
            Self::DEVICE_FEATURE_SELECT => Some(self.device_feature_select),
            Self::DRIVER_FEATURE_SELECT => Some(self.driver_feature_select),
            Self::SHM_SEL => Some(self.shm_sel),
            Self::SHM_LEN_LOW => Some(self.shm_len_low),
            Self::SHM_LEN_HIGH => Some(self.shm_len_high),
            Self::SHM_BASE_LOW => Some(self.shm_base_low),
            Self::SHM_BASE_HIGH => Some(self.shm_base_high),
            _ => None,
        }
    }

    pub fn read_u16(&self, offset: u64) -> Option<u16> {
        match offset {
            Self::CONFIG_MSIX_VECTOR => Some(self.config_msix_vector),
            Self::NUM_QUEUES => Some(self.num_queues),
            Self::QUEUE_SELECT => Some(self.queue_select),
            Self::QUEUE_SIZE => Some(self.queue_size),
            Self::QUEUE_MSIX_VECTOR => Some(self.queue_msix_vector),
            Self::QUEUE_ENABLE => Some(self.queue_enable),
            Self::QUEUE_NOTIFY_OFF => Some(self.queue_notify_off),
            Self::QUEUE_NOTIFY_DATA => Some(self.queue_notify_data),
            Self::QUEUE_RESET => Some(self.queue_reset),
            _ => None,
        }
    }

    pub fn read_u8(&self, offset: u64) -> Option<u8> {
        match offset {
            Self::DEVICE_STATUS => Some(self.device_status),
            Self::CONFIG_GENERATION => Some(self.config_generation),
            _ => None,
        }
    }

    pub fn read_u64(&self, offset: u64) -> Option<u64> {
        match offset {
            Self::QUEUE_DESC => Some(self.queue_desc),
            Self::QUEUE_DRIVER => Some(self.queue_driver),
            Self::QUEUE_DEVICE => Some(self.queue_device),
            _ => None,
        }
    }

    pub fn write_u32(&mut self, offset: u64, value: u32) -> bool {
        match offset {
            Self::DEVICE_FEATURE_SELECT => {
                self.device_feature_select = value;
                true
            }
            Self::DRIVER_FEATURE_SELECT => {
                self.driver_feature_select = value;
                true
            }
            Self::SHM_SEL => {
                self.shm_sel = value;
                true
            }
            _ => false,
        }
    }

    pub fn write_u16(&mut self, offset: u64, value: u16) -> bool {
        match offset {
            Self::CONFIG_MSIX_VECTOR => {
                self.config_msix_vector = value;
                true
            }
            Self::QUEUE_SELECT => {
                self.queue_select = value;
                true
            }
            Self::QUEUE_SIZE => {
                self.queue_size = value;
                true
            }
            Self::QUEUE_MSIX_VECTOR => {
                self.queue_msix_vector = value;
                true
            }
            Self::QUEUE_ENABLE => {
                self.queue_enable = value;
                true
            }
            Self::QUEUE_RESET => {
                if value != 1 {
                    return false;
                }
                self.reset_queue();
                true
            }
            Self::NUM_QUEUES | Self::QUEUE_NOTIFY_OFF | Self::QUEUE_NOTIFY_DATA => false,
            _ => false,
        }
    }

    pub fn write_u8(&mut self, offset: u64, value: u8) -> bool {
        match offset {
            Self::DEVICE_STATUS => {
                self.device_status = value;
                true
            }
            Self::CONFIG_GENERATION => false,
            _ => false,
        }
    }

    pub fn write_u64(&mut self, offset: u64, value: u64) -> bool {
        match offset {
            Self::QUEUE_DESC => {
                self.queue_desc = value;
                true
            }
            Self::QUEUE_DRIVER => {
                self.queue_driver = value;
                true
            }
            Self::QUEUE_DEVICE => {
                self.queue_device = value;
                true
            }
            _ => false,
        }
    }
}

impl Default for CommonConfig {
    fn default() -> Self {
        Self::new(2, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offsets_match_modern_virtio_pci_layout() {
        assert_eq!(CommonConfig::DEVICE_FEATURE_SELECT, 0x00);
        assert_eq!(CommonConfig::DEVICE_FEATURE, 0x04);
        assert_eq!(CommonConfig::DRIVER_FEATURE_SELECT, 0x08);
        assert_eq!(CommonConfig::DRIVER_FEATURE, 0x0c);
        assert_eq!(CommonConfig::CONFIG_MSIX_VECTOR, 0x10);
        assert_eq!(CommonConfig::NUM_QUEUES, 0x12);
        assert_eq!(CommonConfig::DEVICE_STATUS, 0x14);
        assert_eq!(CommonConfig::QUEUE_SELECT, 0x16);
        assert_eq!(CommonConfig::QUEUE_SIZE, 0x18);
        assert_eq!(CommonConfig::QUEUE_MSIX_VECTOR, 0x1a);
        assert_eq!(CommonConfig::QUEUE_ENABLE, 0x1c);
        assert_eq!(CommonConfig::QUEUE_NOTIFY_OFF, 0x1e);
        assert_eq!(CommonConfig::QUEUE_DESC, 0x20);
        assert_eq!(CommonConfig::QUEUE_DRIVER, 0x28);
        assert_eq!(CommonConfig::QUEUE_DEVICE, 0x30);
        assert_eq!(CommonConfig::QUEUE_NOTIFY_DATA, 0x38);
        assert_eq!(CommonConfig::SHM_SEL, 0xac);
        assert_eq!(CommonConfig::SHM_LEN_LOW, 0xb0);
        assert_eq!(CommonConfig::SHM_LEN_HIGH, 0xb4);
        assert_eq!(CommonConfig::SHM_BASE_LOW, 0xb8);
        assert_eq!(CommonConfig::SHM_BASE_HIGH, 0xbc);
        assert_eq!(CommonConfig::QUEUE_RESET, 0xc0);
        assert_eq!(CommonConfig::CONFIG_GENERATION, 0xfc);
        assert_eq!(CommonConfig::SIZE, 0x100);
    }

    #[test]
    fn shared_memory_registers_round_trip() {
        let mut config = CommonConfig::new(2, 7);
        config.set_shared_memory_region(0x1122_3344_5566_7788, 0x99aa_bbcc_ddee_ff00);
        assert_eq!(config.shm_len_low, 0x5566_7788);
        assert_eq!(config.shm_len_high, 0x1122_3344);
        assert_eq!(config.shm_base_low, 0xddee_ff00);
        assert_eq!(config.shm_base_high, 0x99aa_bbcc);
    }

    #[test]
    fn invalid_shared_memory_selection_uses_all_ones() {
        let mut config = CommonConfig::new(2, 0);
        config.set_no_shared_memory_region();
        assert_eq!(config.shm_len_low, u32::MAX);
        assert_eq!(config.shm_len_high, u32::MAX);
        assert_eq!(config.shm_base_low, u32::MAX);
        assert_eq!(config.shm_base_high, u32::MAX);
    }

    #[test]
    fn queue_reset_clears_queue_state() {
        let mut config = CommonConfig {
            queue_size: 256,
            queue_enable: 1,
            queue_desc: 0x1000,
            queue_driver: 0x2000,
            queue_device: 0x3000,
            ..Default::default()
        };
        assert!(config.write_u16(CommonConfig::QUEUE_RESET, 1));
        assert_eq!(config.queue_size, 0);
        assert_eq!(config.queue_enable, 0);
        assert_eq!(config.queue_desc, 0);
        assert_eq!(config.queue_driver, 0);
        assert_eq!(config.queue_device, 0);
    }
}
