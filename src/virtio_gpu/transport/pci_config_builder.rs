use crate::virtio_gpu::transport::PciConfigSpace;
use crate::virtio_gpu::transport::pci::{
    PCI_DEVICE_ID_GPU, PCI_VENDOR_ID_VIRTIO, PciTransportError, VIRTIO_PCI_CAP_NOTIFY_CFG,
    VIRTIO_PCI_CAP_PCI_CFG, VirtioPciNotifyCapability, VirtioPciTransport,
};
use crate::virtio_gpu::transport::pci_config_space::VirtioPciCfgCapability;

#[derive(Debug, PartialEq, Eq)]
pub enum PciCfgAccessError {
    InvalidLength,
    Misaligned,
    InvalidBar,
    OutsideAdvertisedRegion,
    Transport(PciTransportError),
}

impl From<PciTransportError> for PciCfgAccessError {
    fn from(value: PciTransportError) -> Self {
        Self::Transport(value)
    }
}

pub fn build_pci_config_space(
    transport: &VirtioPciTransport,
) -> Result<PciConfigSpace, &'static str> {
    let mut config = PciConfigSpace::new(PCI_VENDOR_ID_VIRTIO, PCI_DEVICE_ID_GPU);

    for index in 0..6u8 {
        if let Some(bar) = transport.bar(index) {
            if !config.set_bar_memory(index, bar.base) {
                return Err("BAR address cannot be represented in conventional PCI space");
            }
        }
    }

    let mut common = Vec::new();
    let mut notify = Vec::new();
    for cap in transport.capabilities().iter().copied() {
        if cap.cfg_type == VIRTIO_PCI_CAP_NOTIFY_CFG {
            notify.push(VirtioPciNotifyCapability::new(
                cap.bar,
                cap.offset,
                cap.length,
                transport.notify_off_multiplier(),
            ));
        } else {
            common.push(cap);
        }
    }

    let pci_cfg = VirtioPciCfgCapability::new(0, 0x300, 4);
    config.install_capabilities_with_pci_cfg(
        &common,
        &notify,
        transport.shared_memory_capabilities(),
        Some(pci_cfg),
    )?;
    Ok(config)
}

fn validate_target(
    transport: &VirtioPciTransport,
    cfg: &VirtioPciCfgCapability,
) -> Result<usize, PciCfgAccessError> {
    let length = cfg
        .access_length()
        .ok_or(PciCfgAccessError::InvalidLength)?;
    if !cfg.cap.offset.is_multiple_of(length as u32) {
        return Err(PciCfgAccessError::Misaligned);
    }

    let bar = cfg.cap.bar;
    let offset = u64::from(cfg.cap.offset);
    let length_u64 = length as u64;

    let advertised = transport
        .capabilities()
        .iter()
        .filter(|cap| cap.bar == bar && cap.cfg_type != VIRTIO_PCI_CAP_PCI_CFG)
        .any(|cap| {
            offset >= u64::from(cap.offset)
                && offset
                    .checked_add(length_u64)
                    .is_some_and(|end| end <= u64::from(cap.offset) + u64::from(cap.length))
        });

    let shared = transport.shared_memory_capabilities().iter().any(|cap| {
        cap.cap.bar == bar
            && offset >= cap.offset()
            && offset
                .checked_add(length_u64)
                .is_some_and(|end| end <= cap.offset() + cap.length())
    });

    if !advertised && !shared {
        return Err(PciCfgAccessError::OutsideAdvertisedRegion);
    }

    Ok(length)
}

impl VirtioPciTransport {
    pub fn build_pci_config_space(&self) -> Result<PciConfigSpace, &'static str> {
        build_pci_config_space(self)
    }

    pub fn pci_cfg_read(&self, cfg: &VirtioPciCfgCapability) -> Result<[u8; 4], PciCfgAccessError> {
        let length = validate_target(self, cfg)?;
        let bar = cfg.cap.bar;
        let offset = u64::from(cfg.cap.offset);

        let value = if bar == 0 {
            if offset < crate::virtio_gpu::transport::pci::common::CommonConfig::SIZE {
                self.read_common(offset, length)?
            } else if offset >= 0x300 {
                self.read_device_config(offset - 0x300, length)?
            } else {
                return Err(PciCfgAccessError::OutsideAdvertisedRegion);
            }
        } else {
            let region = self
                .shared_memory_capabilities()
                .iter()
                .copied()
                .find(|cap| {
                    cap.cap.bar == bar
                        && offset >= cap.offset()
                        && offset + length as u64 <= cap.offset() + cap.length()
                })
                .ok_or(PciCfgAccessError::InvalidBar)?;
            let relative = usize::try_from(offset - region.offset())
                .map_err(|_| PciCfgAccessError::InvalidLength)?;
            let bytes = self.shared_memory_region_bytes(u32::from(region.cap.id))?;
            let end = relative
                .checked_add(length)
                .ok_or(PciCfgAccessError::InvalidLength)?;
            if end > bytes.len() {
                return Err(PciCfgAccessError::OutsideAdvertisedRegion);
            }
            let mut raw = [0u8; 8];
            raw[..length].copy_from_slice(&bytes[relative..end]);
            u64::from_le_bytes(raw)
        };

        let mut out = [0u8; 4];
        out[..length].copy_from_slice(&value.to_le_bytes()[..length]);
        Ok(out)
    }

    pub fn pci_cfg_write(
        &mut self,
        cfg: &VirtioPciCfgCapability,
        data: &[u8],
    ) -> Result<(), PciCfgAccessError> {
        let length = validate_target(self, cfg)?;
        if data.len() != length {
            return Err(PciCfgAccessError::InvalidLength);
        }
        let mut raw = [0u8; 8];
        raw[..length].copy_from_slice(data);
        let value = u64::from_le_bytes(raw);
        let bar = cfg.cap.bar;
        let offset = u64::from(cfg.cap.offset);

        if bar == 0 {
            if offset < crate::virtio_gpu::transport::pci::common::CommonConfig::SIZE {
                self.write_common(offset, length, value)?;
            } else if offset >= 0x300 {
                self.write_device_config(offset - 0x300, length, value)?;
            } else {
                return Err(PciCfgAccessError::OutsideAdvertisedRegion);
            }
            return Ok(());
        }

        let region = self
            .shared_memory_capabilities()
            .iter()
            .copied()
            .find(|cap| {
                cap.cap.bar == bar
                    && offset >= cap.offset()
                    && offset + length as u64 <= cap.offset() + cap.length()
            })
            .ok_or(PciCfgAccessError::InvalidBar)?;
        let relative = usize::try_from(offset - region.offset())
            .map_err(|_| PciCfgAccessError::InvalidLength)?;
        let bytes = self.shared_memory_region_bytes_mut(u32::from(region.cap.id))?;
        let end = relative
            .checked_add(length)
            .ok_or(PciCfgAccessError::InvalidLength)?;
        if end > bytes.len() {
            return Err(PciCfgAccessError::OutsideAdvertisedRegion);
        }
        bytes[relative..end].copy_from_slice(data);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_contains_required_capabilities() {
        let mut transport = VirtioPciTransport::default();
        transport.initialize_default_capabilities().unwrap();
        let config = transport.build_pci_config_space().unwrap();

        let mut cursor = usize::from(config.capability_pointer());
        let mut seen_common = false;
        let mut seen_shared = false;
        let mut seen_pci_cfg = false;

        for _ in 0..32 {
            let cfg_type = config.read(cursor + 3, 1).unwrap() as u8;
            seen_common |= cfg_type == crate::virtio_gpu::transport::pci::VIRTIO_PCI_CAP_COMMON_CFG;
            seen_shared |=
                cfg_type == crate::virtio_gpu::transport::pci::VIRTIO_PCI_CAP_SHARED_MEMORY_CFG;
            seen_pci_cfg |= cfg_type == VIRTIO_PCI_CAP_PCI_CFG;
            let next = config.read(cursor + 1, 1).unwrap() as usize;
            if next == 0 {
                break;
            }
            cursor = next;
        }

        assert!(seen_common);
        assert!(seen_shared);
        assert!(seen_pci_cfg);
    }

    #[test]
    fn pci_cfg_can_read_and_write_device_config() {
        let mut transport = VirtioPciTransport::default();
        transport.initialize_default_capabilities().unwrap();
        let cfg = VirtioPciCfgCapability::new(0, 0x300, 4);
        transport
            .pci_cfg_write(&cfg, &[0xde, 0xad, 0xbe, 0xef])
            .unwrap();
        assert_eq!(
            &transport.pci_cfg_read(&cfg).unwrap()[..4],
            &[0xde, 0xad, 0xbe, 0xef]
        );
    }

    #[test]
    fn pci_cfg_can_read_and_write_shared_memory() {
        let mut transport = VirtioPciTransport::default();
        transport.initialize_default_capabilities().unwrap();
        let cfg = VirtioPciCfgCapability::new(2, 0x100, 4);
        transport.pci_cfg_write(&cfg, &[1, 2, 3, 4]).unwrap();
        assert_eq!(&transport.pci_cfg_read(&cfg).unwrap()[..4], &[1, 2, 3, 4]);
    }

    #[test]
    fn pci_cfg_rejects_unadvertised_ranges() {
        let transport = VirtioPciTransport::default();
        let cfg = VirtioPciCfgCapability::new(2, 0x3ff0, 4);
        assert_eq!(
            transport.pci_cfg_read(&cfg),
            Err(PciCfgAccessError::OutsideAdvertisedRegion)
        );
    }
}
