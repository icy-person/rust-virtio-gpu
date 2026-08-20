use rust_virtio_gpu::virtio_gpu::device::DeviceStatus;
use rust_virtio_gpu::virtio_gpu::features::GpuFeatures;
use rust_virtio_gpu::virtio_gpu::transport::pci::common::CommonConfig;
use rust_virtio_gpu::virtio_gpu::transport::pci::{
    PciTransportError, VirtioPciCap64, VirtioPciCapability, VirtioPciNotifyCapability,
    VirtioPciTransport, VIRTIO_PCI_CAP_COMMON_CFG, VIRTIO_PCI_CAP_DEVICE_CFG,
    VIRTIO_PCI_CAP_NOTIFY_CFG, VIRTIO_PCI_CAP_SHARED_MEMORY_CFG,
};

#[test]
fn default_transport_advertises_shared_memory() {
    let mut transport = VirtioPciTransport::default();
    transport.initialize_default_capabilities().unwrap();

    assert!(transport.capability(VIRTIO_PCI_CAP_COMMON_CFG).is_some());
    assert!(transport.capability(VIRTIO_PCI_CAP_NOTIFY_CFG).is_some());
    assert!(transport.capability(VIRTIO_PCI_CAP_DEVICE_CFG).is_some());
    assert_eq!(transport.shared_memory_capabilities().len(), 1);

    let cap = transport
        .shared_memory_capability(1)
        .expect("shared-memory capability id 1");
    assert_eq!(cap.cap.cfg_type, VIRTIO_PCI_CAP_SHARED_MEMORY_CFG);
    assert_eq!(cap.cap.id, 1);
    assert_eq!(cap.offset(), 0);
    assert_eq!(cap.length(), 0x0400_0000);
}

#[test]
fn shared_memory_selection_exposes_length_and_base() {
    let mut transport = VirtioPciTransport::default();
    transport.initialize_default_capabilities().unwrap();
    transport
        .write_common(CommonConfig::SHM_SEL, 4, 1)
        .unwrap();

    assert_eq!(
        transport.read_common(CommonConfig::SHM_LEN_LOW, 4).unwrap(),
        0x0400_0000
    );
    assert_eq!(
        transport.read_common(CommonConfig::SHM_LEN_HIGH, 4).unwrap(),
        0
    );
    assert_eq!(
        transport.read_common(CommonConfig::SHM_BASE_LOW, 4).unwrap(),
        0x2000_0000
    );
    assert_eq!(
        transport.read_common(CommonConfig::SHM_BASE_HIGH, 4).unwrap(),
        0
    );
}

#[test]
fn nonexistent_shared_memory_is_reported_as_all_ones() {
    let mut transport = VirtioPciTransport::default();
    transport.initialize_default_capabilities().unwrap();
    transport
        .write_common(CommonConfig::SHM_SEL, 4, 0xfeed)
        .unwrap();

    assert_eq!(
        transport.read_common(CommonConfig::SHM_LEN_LOW, 4).unwrap(),
        u32::MAX as u64
    );
    assert_eq!(
        transport.read_common(CommonConfig::SHM_LEN_HIGH, 4).unwrap(),
        u32::MAX as u64
    );
    assert_eq!(
        transport.read_common(CommonConfig::SHM_BASE_LOW, 4).unwrap(),
        u32::MAX as u64
    );
    assert_eq!(
        transport.read_common(CommonConfig::SHM_BASE_HIGH, 4).unwrap(),
        u32::MAX as u64
    );
}

#[test]
fn shared_memory_storage_is_mutable_and_bounds_checked() {
    let mut transport = VirtioPciTransport::default();
    transport.initialize_default_capabilities().unwrap();

    let bytes = transport
        .shared_memory_region_bytes_mut(1)
        .expect("shared-memory storage");
    bytes[..8].copy_from_slice(&0x1122_3344_5566_7788u64.to_le_bytes());

    assert_eq!(
        transport.shared_memory_region_bytes(1).unwrap()[..8],
        0x1122_3344_5566_7788u64.to_le_bytes()
    );
    assert_eq!(
        transport.shared_memory_region_bytes(99),
        Err(PciTransportError::SharedMemoryNotFound)
    );
}

#[test]
fn capability_wire_round_trips_match_struct_layouts() {
    let base = VirtioPciCapability::new_with_id(
        VIRTIO_PCI_CAP_SHARED_MEMORY_CFG,
        2,
        17,
        0x1234_0000,
        0x2000,
    );
    assert_eq!(
        VirtioPciCapability::decode_le(&base.encode_le()),
        Some(base)
    );

    let cap64 = VirtioPciCap64::new(2, 17, 0x1_0000_1234, 0x2_0000_5678);
    assert_eq!(VirtioPciCap64::decode_le(&cap64.encode_le()), Some(cap64));
    assert_eq!(cap64.offset(), 0x1_0000_1234);
    assert_eq!(cap64.length(), 0x2_0000_5678);

    let notify = VirtioPciNotifyCapability::new(0, 0x100, 0x100, 4);
    assert_eq!(
        VirtioPciNotifyCapability::decode_le(&notify.encode_le()),
        Some(notify)
    );
}

#[test]
fn feature_and_status_registers_round_trip() {
    let mut transport = VirtioPciTransport::default();
    let low = transport
        .read_common(CommonConfig::DEVICE_FEATURE, 4)
        .unwrap();
    let offered = transport.device.device_features();
    assert_eq!(low, offered.bits() & u64::from(u32::MAX));

    transport
        .write_common(CommonConfig::DRIVER_FEATURE, 4, low)
        .unwrap();
    assert_eq!(
        transport.device.driver_features(),
        GpuFeatures::from_bits_truncate(low)
    );

    transport
        .write_common(
            CommonConfig::DEVICE_STATUS,
            1,
            DeviceStatus::ACKNOWLEDGE.bits() as u64,
        )
        .unwrap();
    assert!(transport.device.status().contains(DeviceStatus::ACKNOWLEDGE));
}

#[test]
fn queue_reset_register_clears_selected_queue() {
    let mut transport = VirtioPciTransport::default();
    transport
        .write_common(CommonConfig::QUEUE_SELECT, 2, 0)
        .unwrap();
    transport
        .write_common(CommonConfig::QUEUE_SIZE, 2, 256)
        .unwrap();
    assert_eq!(
        transport.read_common(CommonConfig::QUEUE_SIZE, 2).unwrap(),
        256
    );

    transport
        .write_common(CommonConfig::QUEUE_RESET, 2, 1)
        .unwrap();
    assert_eq!(
        transport.read_common(CommonConfig::QUEUE_SIZE, 2).unwrap(),
        0
    );
    assert_eq!(
        transport.read_common(CommonConfig::QUEUE_ENABLE, 2).unwrap(),
        0
    );
}
