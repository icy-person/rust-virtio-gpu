use crate::virtio_gpu::transport::PciConfigSpace;
use crate::virtio_gpu::transport::pci::{
    PCI_DEVICE_ID_GPU, PCI_VENDOR_ID_VIRTIO, VIRTIO_PCI_CAP_NOTIFY_CFG, VirtioPciNotifyCapability,
    VirtioPciTransport,
};
use crate::virtio_gpu::transport::pci_config_space::VirtioPciCfgCapability;

/// Build a conventional 4 KiB PCI configuration-space image for the current
/// VirtIO PCI transport state. The capability list includes all common/notify/
/// ISR/device capabilities, all shared-memory regions, and the mandatory PCI
/// configuration-access capability.
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

    // The capability initially points at the beginning of the device-specific
    // configuration window. The driver may rewrite BAR/offset/length later.
    let pci_cfg = VirtioPciCfgCapability::new(0, 0x300, 4);

    config.install_capabilities_with_pci_cfg(
        &common,
        &notify,
        transport.shared_memory_capabilities(),
        Some(pci_cfg),
    )?;

    Ok(config)
}

impl VirtioPciTransport {
    pub fn build_pci_config_space(&self) -> Result<PciConfigSpace, &'static str> {
        build_pci_config_space(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::virtio_gpu::transport::pci::{
        VIRTIO_PCI_CAP_COMMON_CFG, VIRTIO_PCI_CAP_PCI_CFG, VIRTIO_PCI_CAP_SHARED_MEMORY_CFG,
    };

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
            seen_common |= cfg_type == VIRTIO_PCI_CAP_COMMON_CFG;
            seen_shared |= cfg_type == VIRTIO_PCI_CAP_SHARED_MEMORY_CFG;
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
}
