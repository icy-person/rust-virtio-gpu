use bitflags::bitflags;

use crate::virtio_gpu::features::GpuFeatures;

use crate::virtio_gpu::display::scanout::Scanout;

use crate::virtio_gpu::protocol::commands::{
    CMD_GET_DISPLAY_INFO, CMD_RESOURCE_ATTACH_BACKING, CMD_RESOURCE_CREATE_2D, CMD_RESOURCE_FLUSH,
    CMD_SET_SCANOUT, CMD_TRANSFER_TO_HOST_2D, CONTROLQ, CURSORQ, RESP_OK_DISPLAY_INFO,
};
use crate::virtio_gpu::protocol::config::GpuConfig;
use crate::virtio_gpu::transport::GuestMemory;
use crate::virtio_gpu::transport::memory::GuestAddress;
use crate::virtio_gpu::transport::memory::GuestMemoryError;
use crate::virtio_gpu::transport::virtqueue::split::DescriptorChain;
use crate::virtio_gpu::transport::virtqueue::{SplitVirtQueue, VirtQueueError};

use crate::virtio_gpu::protocol::header::CtrlHeader;

use crate::virtio_gpu::protocol::responses::{
    DisplayOne, MAX_SCANOUTS, Rect, RespDisplayInfo, RespOkNoData,
};

use crate::virtio_gpu::transport::virtqueue::split::DESC_F_WRITE;

use crate::virtio_gpu::protocol::requests::attach_backing::ResourceAttachBacking;
use crate::virtio_gpu::protocol::requests::flush::ResourceFlush;
use crate::virtio_gpu::protocol::requests::scanout::ResourceSetScanout;
use crate::virtio_gpu::protocol::requests::transfer::ResourceTransferToHost2D;
use crate::virtio_gpu::protocol::resource::ResourceCreate2D;
use crate::virtio_gpu::resource::Resource;
use crate::virtio_gpu::resource::ResourceTable;

use crate::virtio_gpu::renderer::{Display, VulkanRenderer};
use crate::virtio_gpu::renderer::{Renderer, SoftwareRenderer};

pub const DEVICE_ID: u32 = 16;

pub const STATUS_ACKNOWLEDGE: u8 = 1 << 0;
pub const STATUS_DRIVER: u8 = 1 << 1;
pub const STATUS_DRIVER_OK: u8 = 1 << 2;
pub const STATUS_FEATURES_OK: u8 = 1 << 3;
pub const STATUS_DEVICE_NEEDS_RESET: u8 = 1 << 6;
pub const STATUS_FAILED: u8 = 1 << 7;

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct DeviceStatus: u8 {
        const ACKNOWLEDGE = STATUS_ACKNOWLEDGE;
        const DRIVER = STATUS_DRIVER;
        const DRIVER_OK = STATUS_DRIVER_OK;
        const FEATURES_OK = STATUS_FEATURES_OK;
        const DEVICE_NEEDS_RESET = STATUS_DEVICE_NEEDS_RESET;
        const FAILED = STATUS_FAILED;
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum DeviceError {
    InvalidStatusTransition,
    FeaturesNotSupported,
    FeaturesAlreadyNegotiated,
    QueueIndexInvalid,
    QueueAlreadyConfigured,
    QueueNotReady,
    InvalidCommand,
    InvalidDescriptor,
    InvalidRequest,
    InvalidFormat,
    InvalidResource,
    InvalidParameter,
    UnsupportedCommand,
    ResponseBufferTooSmall,
    ResourceExists,
    Memory(GuestMemoryError),
    VirtQueue(VirtQueueError),
}

impl From<VirtQueueError> for DeviceError {
    fn from(value: VirtQueueError) -> Self {
        Self::VirtQueue(value)
    }
}

impl From<GuestMemoryError> for DeviceError {
    fn from(value: GuestMemoryError) -> Self {
        Self::Memory(value)
    }
}

pub struct VirtioGpuDevice {
    device_features: GpuFeatures,
    driver_features: GpuFeatures,

    status: DeviceStatus,

    config: GpuConfig,
    config_generation: u8,

    controlq: Option<SplitVirtQueue>,
    cursorq: Option<SplitVirtQueue>,

    memory: GuestMemory,
    resource: ResourceTable,

    scanouts: [Scanout; MAX_SCANOUTS],

    pub renderer: Option<Box<dyn Renderer>>,
    pub display: Option<Display>,
}

impl VirtioGpuDevice {
    pub fn new() -> Self {
        Self {
            device_features: GpuFeatures::VIRGL
                | GpuFeatures::EDID
                | GpuFeatures::RESOURCE_UUID
                | GpuFeatures::RESOURCE_BLOB
                | GpuFeatures::CONTEXT_INIT
                | GpuFeatures::BLOB_ALIGNMENT,

            driver_features: GpuFeatures::empty(),

            status: DeviceStatus::empty(),

            config: GpuConfig {
                events_read: 0,
                events_clear: 0,
                num_scanouts: 1,
                num_capsets: 1,
                blob_alignment: Some(4096),
            },

            config_generation: 0,

            controlq: None,
            cursorq: None,
            memory: GuestMemory::new(GuestAddress::new(0), 16 * 1024 * 1024),
            resource: ResourceTable::new(),
            scanouts: [Scanout::default(); MAX_SCANOUTS],
            renderer: Some(Box::new(VulkanRenderer::new(1920, 1080))),
            display: None,
        }
    }

    pub fn process_queue(&mut self) -> Result<(), DeviceError> {
        self.require_ready()?;

        loop {
            let chain = {
                let queue = self.controlq.as_mut().ok_or(DeviceError::QueueNotReady)?;

                queue.pop_chain()?
            };

            match chain {
                Some(chain) => {
                    self.process_command(chain)?;
                }
                None => break,
            }
        }

        Ok(())
    }
    fn process_command(&mut self, chain: DescriptorChain) -> Result<(), DeviceError> {
        let request = {
            let queue = self.controlq.as_ref().ok_or(DeviceError::QueueNotReady)?;
            queue.read_chain(&chain)?
        };

        let header = CtrlHeader::decode_le(&request[..CtrlHeader::SIZE])
            .ok_or(DeviceError::InvalidRequest)?;

        let response_len = match header.typ {
            CMD_RESOURCE_CREATE_2D => {
                let req = ResourceCreate2D::decode(&request)?;

                self.create_resource_2d(req)?;

                let response = RespOkNoData::new();
                let bytes = response.encode_le();

                self.write_response(&chain, &bytes)?;

                bytes.len() as u32
            }

            CMD_GET_DISPLAY_INFO => {
                let response = self.get_display_info();

                let bytes = response.encode_le();

                self.write_response(&chain, &bytes)?;

                bytes.len() as u32
            }

            CMD_RESOURCE_ATTACH_BACKING => {
                let req =
                    ResourceAttachBacking::decode(&request).ok_or(DeviceError::InvalidRequest)?;

                self.attach_backing(req)?;

                let response = RespOkNoData::new();
                let bytes = response.encode_le();

                self.write_response(&chain, &bytes)?;

                bytes.len() as u32
            }

            CMD_TRANSFER_TO_HOST_2D => {
                let req = ResourceTransferToHost2D::decode(&request)
                    .ok_or(DeviceError::InvalidRequest)?;

                self.transfer_to_host(req)?;

                let response = RespOkNoData::new();
                let bytes = response.encode_le();

                self.write_response(&chain, &bytes)?;

                bytes.len() as u32
            }

            CMD_RESOURCE_FLUSH => {
                let req = ResourceFlush::decode(&request).ok_or(DeviceError::InvalidRequest)?;

                self.resource_flush(req)?;

                let response = RespOkNoData::new();
                let bytes = response.encode_le();

                self.write_response(&chain, &bytes)?;

                bytes.len() as u32
            }

            CMD_SET_SCANOUT => {
                let req =
                    ResourceSetScanout::decode(&request).ok_or(DeviceError::InvalidRequest)?;

                self.set_scanout(req)?;

                let response = RespOkNoData::new();
                let bytes = response.encode_le();

                self.write_response(&chain, &bytes)?;

                bytes.len() as u32
            }

            _ => {
                return Err(DeviceError::UnsupportedCommand);
            }
        };

        let queue = self.controlq.as_mut().ok_or(DeviceError::QueueNotReady)?;

        queue.push_used(chain.head as u32, response_len)?;

        Ok(())
    }

    pub fn resource_mut(&mut self, id: u32) -> Option<&mut Resource> {
        self.resource.get_mut(id)
    }

    pub fn render(&mut self) -> Result<(), DeviceError> {
        for scanout in self.scanouts.iter() {
            if !scanout.enabled {
                continue;
            }

            let resource = self
                .resource
                .get(scanout.resource_id)
                .ok_or(DeviceError::InvalidResource)?;

            if resource.dirty.is_some() {
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.upload(&resource.data);
                }
            }
        }

        Ok(())
    }

    pub fn framebuffer(&self, scanout_id: usize) -> Option<&[u8]> {
        if scanout_id >= MAX_SCANOUTS {
            return None;
        }

        let scanout = &self.scanouts[scanout_id];

        if !scanout.enabled {
            return None;
        }

        let resource = self.resource.get(scanout.resource_id)?;

        Some(resource.pixels())
    }

    pub fn set_scanout(&mut self, req: ResourceSetScanout) -> Result<(), DeviceError> {
        let scanout_id = req.scanout_id as usize;

        if scanout_id >= MAX_SCANOUTS {
            return Err(DeviceError::InvalidResource);
        }

        // resource_id = 0 means disable scanout
        if req.resource_id == 0 {
            self.scanouts[scanout_id] = Scanout::default();

            println!("SET_SCANOUT {} disabled", req.scanout_id);

            return Ok(());
        }

        let resource = self
            .resource
            .get(req.resource_id)
            .ok_or(DeviceError::InvalidResource)?;

        self.scanouts[scanout_id] = Scanout {
            enabled: true,
            resource_id: resource.id,
            width: resource.width,
            height: resource.height,
        };

        if self.display.is_none() {
            self.display = Some(Display::new(
                resource.width as usize,
                resource.height as usize,
            ));
        }

        self.renderer = Some(Box::new(SoftwareRenderer::new(1920, 1080)));

        println!(
            "SET_SCANOUT {} -> resource {} ({}x{})",
            req.scanout_id, req.resource_id, resource.width, resource.height
        );

        Ok(())
    }

    pub fn resource_flush(&mut self, req: ResourceFlush) -> Result<(), DeviceError> {
        let resource = self
            .resource
            .get_mut(req.resource_id)
            .ok_or(DeviceError::InvalidResource)?;

        resource.dirty = Some(req.rect);

        let visible = self
            .scanouts
            .iter()
            .any(|s| s.enabled && s.resource_id == req.resource_id);

        if !visible {
            return Ok(());
        }

        if let Some(renderer) = self.renderer.as_mut() {
            renderer.flush_resource(resource)?;

            if let Some(display) = self.display.as_mut() {
                display.update(renderer.framebuffer_mut());
            }
        }

        Ok(())
    }
    pub fn transfer_to_host(&mut self, req: ResourceTransferToHost2D) -> Result<(), DeviceError> {
        let resource = self
            .resource
            .get_mut(req.resource_id)
            .ok_or(DeviceError::InvalidResource)?;

        let bytes_per_pixel = 4usize;

        let transfer_size = req.rect.width as usize * req.rect.height as usize * bytes_per_pixel;

        let mut copied = 0usize;

        for entry in resource.backing.clone() {
            if copied >= transfer_size {
                break;
            }

            let remaining = transfer_size - copied;

            let size = std::cmp::min(entry.length as usize, remaining);

            let mut buffer = vec![0u8; size];

            self.memory
                .read(GuestAddress::new(entry.addr), &mut buffer)
                .map_err(|_| DeviceError::InvalidResource)?;

            let dst_start = copied;
            let dst_end = copied + size;

            if dst_end > resource.data.len() {
                return Err(DeviceError::InvalidResource);
            }

            resource.data[dst_start..dst_end].copy_from_slice(&buffer);

            copied += size;
        }

        if copied != transfer_size {
            return Err(DeviceError::InvalidResource);
        }

        if let Some(renderer) = self.renderer.as_mut() {
            renderer.transfer_resource(resource)?;
        }

        Ok(())
    }

    fn write_response(&mut self, chain: &DescriptorChain, data: &[u8]) -> Result<(), DeviceError> {
        let queue = self.controlq.as_mut().ok_or(DeviceError::QueueNotReady)?;

        let descriptors = queue.descriptor_chain(chain.head)?;

        for (_, desc) in descriptors.iter().skip(1) {
            if desc.flags & DESC_F_WRITE != 0 && desc.len as usize >= data.len() {
                queue.memory_mut().write(desc.addr, data)?;

                return Ok(());
            }
        }

        Err(DeviceError::ResponseBufferTooSmall)
    }

    fn get_display_info(&self) -> RespDisplayInfo {
        let mut pmodes = [DisplayOne::default(); MAX_SCANOUTS];

        for (id, scanout) in self.scanouts.iter().enumerate() {
            if scanout.enabled {
                if let Some(resource) = self.resource.get(scanout.resource_id) {
                    pmodes[id] = DisplayOne {
                        rect: Rect {
                            x: 0,
                            y: 0,
                            width: resource.width,
                            height: resource.height,
                        },
                        enabled: 1,
                        flags: 0,
                    };
                }
            }
        }

        RespDisplayInfo {
            header: CtrlHeader::new(RESP_OK_DISPLAY_INFO),
            pmodes,
        }
    }

    pub fn create_resource_2d(&mut self, request: ResourceCreate2D) -> Result<(), DeviceError> {
        let resource = Resource::new(
            request.resource_id,
            request.width,
            request.height,
            request.format,
        );

        if !self.resource.insert(resource) {
            return Err(DeviceError::ResourceExists);
        }

        Ok(())
    }

    pub fn device_id(&self) -> u32 {
        DEVICE_ID
    }

    pub fn device_features(&self) -> GpuFeatures {
        self.device_features
    }

    pub fn driver_features(&self) -> GpuFeatures {
        self.driver_features
    }

    pub fn status(&self) -> DeviceStatus {
        self.status
    }

    pub fn config(&self) -> GpuConfig {
        self.config
    }

    pub fn config_generation(&self) -> u8 {
        self.config_generation
    }

    pub fn is_driver_ok(&self) -> bool {
        self.status.contains(DeviceStatus::DRIVER_OK)
    }

    pub fn features_ok(&self) -> bool {
        self.status.contains(DeviceStatus::FEATURES_OK)
    }

    pub fn control_queue(&self) -> Option<&SplitVirtQueue> {
        self.controlq.as_ref()
    }

    pub fn control_queue_mut(&mut self) -> Option<&mut SplitVirtQueue> {
        self.controlq.as_mut()
    }

    pub fn cursor_queue(&self) -> Option<&SplitVirtQueue> {
        self.cursorq.as_ref()
    }

    pub fn cursor_queue_mut(&mut self) -> Option<&mut SplitVirtQueue> {
        self.cursorq.as_mut()
    }

    pub fn reset(&mut self) {
        self.status = DeviceStatus::empty();
        self.driver_features = GpuFeatures::empty();

        self.controlq = None;
        self.cursorq = None;

        self.config_generation = self.config_generation.wrapping_add(1);
    }

    pub fn set_status(&mut self, new_status: u8) -> Result<(), DeviceError> {
        if new_status == 0 {
            self.reset();
            return Ok(());
        }

        let requested = DeviceStatus::from_bits_truncate(new_status);

        if requested.contains(DeviceStatus::DRIVER)
            && !requested.contains(DeviceStatus::ACKNOWLEDGE)
        {
            return Err(DeviceError::InvalidStatusTransition);
        }

        if requested.contains(DeviceStatus::FEATURES_OK)
            && !requested.contains(DeviceStatus::DRIVER)
        {
            return Err(DeviceError::InvalidStatusTransition);
        }

        if requested.contains(DeviceStatus::DRIVER_OK)
            && !requested.contains(DeviceStatus::FEATURES_OK)
        {
            return Err(DeviceError::InvalidStatusTransition);
        }

        if requested.contains(DeviceStatus::FEATURES_OK) && !self.features_are_supported() {
            return Err(DeviceError::FeaturesNotSupported);
        }

        self.status = requested;

        Ok(())
    }

    pub fn set_driver_features(&mut self, features: u64) -> Result<(), DeviceError> {
        if self.features_ok() {
            return Err(DeviceError::FeaturesAlreadyNegotiated);
        }

        let requested =
            GpuFeatures::from_bits(features).ok_or(DeviceError::FeaturesNotSupported)?;

        if !self.device_features.contains(requested) {
            return Err(DeviceError::FeaturesNotSupported);
        }

        self.driver_features = requested;

        Ok(())
    }
    pub fn features_are_supported(&self) -> bool {
        if !self.device_features.contains(self.driver_features) {
            return false;
        }

        if self.driver_features.contains(GpuFeatures::CONTEXT_INIT)
            && !self.driver_features.contains(GpuFeatures::VIRGL)
        {
            return false;
        }

        if self.driver_features.contains(GpuFeatures::BLOB_ALIGNMENT)
            && !self.driver_features.contains(GpuFeatures::RESOURCE_BLOB)
        {
            return false;
        }

        true
    }
    pub fn configure_queue(
        &mut self,
        queue_index: u16,
        base: GuestAddress,
        queue_size: u16,
    ) -> Result<(), DeviceError> {
        if !self.status.contains(DeviceStatus::DRIVER) {
            return Err(DeviceError::QueueNotReady);
        }

        if self.features_ok() {
            // Queue configuration is still allowed before DRIVER_OK.
        }

        let queue = SplitVirtQueue::new(self.memory.clone(), base, queue_size)?;

        match queue_index {
            CONTROLQ => {
                if self.controlq.is_some() {
                    return Err(DeviceError::QueueAlreadyConfigured);
                }

                self.controlq = Some(queue);
            }

            CURSORQ => {
                if self.cursorq.is_some() {
                    return Err(DeviceError::QueueAlreadyConfigured);
                }

                self.cursorq = Some(queue);
            }

            _ => return Err(DeviceError::QueueIndexInvalid),
        }

        Ok(())
    }

    pub fn queue_ready(&self, queue_index: u16) -> Result<bool, DeviceError> {
        match queue_index {
            CONTROLQ => Ok(self.controlq.is_some()),
            CURSORQ => Ok(self.cursorq.is_some()),
            _ => Err(DeviceError::QueueIndexInvalid),
        }
    }

    pub fn require_ready(&self) -> Result<(), DeviceError> {
        if !self.is_driver_ok() {
            return Err(DeviceError::QueueNotReady);
        }

        if self.controlq.is_none() {
            return Err(DeviceError::QueueNotReady);
        }

        Ok(())
    }

    fn attach_backing(&mut self, req: ResourceAttachBacking) -> Result<(), DeviceError> {
        self.resource
            .attach_backing(req.resource_id, req.entries)
            .map_err(|_| DeviceError::InvalidResource)?;

        Ok(())
    }

    pub fn memory(&self) -> &GuestMemory {
        &self.memory
    }

    pub fn render_frame(&mut self) {
        if let (Some(renderer), Some(display)) = (self.renderer.as_mut(), self.display.as_mut()) {
            display.update(renderer.framebuffer_mut());
        }
    }

    pub fn present(&mut self) {
        if let (Some(renderer), Some(display)) = (self.renderer.as_mut(), self.display.as_mut()) {
            display.update(renderer.framebuffer_mut());
        }
    }
}

impl Default for VirtioGpuDevice {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::virtio_gpu::protocol::commands::{CMD_RESOURCE_CREATE_2D, RESP_OK_NODATA};
    use crate::virtio_gpu::protocol::formats::VirtioGpuFormat;
    use crate::virtio_gpu::protocol::requests::VirtioGpuMemEntry;

    #[test]
    fn device_starts_in_reset_state() {
        let device = VirtioGpuDevice::new();

        assert_eq!(device.device_id(), 16);
        assert_eq!(device.status(), DeviceStatus::empty());
        assert_eq!(device.driver_features(), GpuFeatures::empty());
        assert!(!device.is_driver_ok());
    }

    #[test]
    fn device_reports_gpu_features() {
        let device = VirtioGpuDevice::new();

        let features = device.device_features();

        assert!(features.contains(GpuFeatures::VIRGL));
        assert!(features.contains(GpuFeatures::RESOURCE_BLOB));
        assert!(features.contains(GpuFeatures::CONTEXT_INIT));
    }

    #[test]
    fn acknowledge_status_is_accepted() {
        let mut device = VirtioGpuDevice::new();

        device.set_status(STATUS_ACKNOWLEDGE).unwrap();

        assert!(device.status().contains(DeviceStatus::ACKNOWLEDGE));
    }

    #[test]
    fn driver_requires_acknowledge() {
        let mut device = VirtioGpuDevice::new();

        assert_eq!(
            device.set_status(STATUS_DRIVER),
            Err(DeviceError::InvalidStatusTransition)
        );
    }

    #[test]
    fn driver_status_is_accepted_after_acknowledge() {
        let mut device = VirtioGpuDevice::new();

        device
            .set_status(STATUS_ACKNOWLEDGE | STATUS_DRIVER)
            .unwrap();

        assert!(device.status().contains(DeviceStatus::DRIVER));
    }

    #[test]
    fn unsupported_features_are_rejected() {
        let mut device = VirtioGpuDevice::new();

        device
            .set_status(STATUS_ACKNOWLEDGE | STATUS_DRIVER)
            .unwrap();

        let unsupported = 1u64 << 63;

        assert_eq!(
            device.set_driver_features(unsupported),
            Err(DeviceError::FeaturesNotSupported)
        );
    }

    #[test]
    fn venus_features_can_be_negotiated() {
        let mut device = VirtioGpuDevice::new();

        device
            .set_status(STATUS_ACKNOWLEDGE | STATUS_DRIVER)
            .unwrap();

        let features =
            (GpuFeatures::VIRGL | GpuFeatures::RESOURCE_BLOB | GpuFeatures::CONTEXT_INIT).bits();

        device.set_driver_features(features).unwrap();

        assert_eq!(
            device.driver_features(),
            GpuFeatures::VIRGL | GpuFeatures::RESOURCE_BLOB | GpuFeatures::CONTEXT_INIT
        );
    }

    #[test]
    fn context_init_requires_virgl() {
        let mut device = VirtioGpuDevice::new();

        device
            .set_status(STATUS_ACKNOWLEDGE | STATUS_DRIVER)
            .unwrap();

        device
            .set_driver_features(GpuFeatures::CONTEXT_INIT.bits())
            .unwrap();

        assert!(!device.features_are_supported());
    }

    #[test]
    fn features_ok_requires_valid_features() {
        let mut device = VirtioGpuDevice::new();

        device
            .set_status(STATUS_ACKNOWLEDGE | STATUS_DRIVER)
            .unwrap();

        device
            .set_driver_features(
                (GpuFeatures::VIRGL | GpuFeatures::RESOURCE_BLOB | GpuFeatures::CONTEXT_INIT)
                    .bits(),
            )
            .unwrap();

        device
            .set_status(STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_FEATURES_OK)
            .unwrap();

        assert!(device.features_ok());
    }

    #[test]
    fn driver_ok_requires_features_ok() {
        let mut device = VirtioGpuDevice::new();

        device
            .set_status(STATUS_ACKNOWLEDGE | STATUS_DRIVER)
            .unwrap();

        assert_eq!(
            device.set_status(STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_DRIVER_OK),
            Err(DeviceError::InvalidStatusTransition)
        );
    }

    #[test]
    fn queue_configuration_works() {
        let mut device = VirtioGpuDevice::new();

        device
            .set_status(STATUS_ACKNOWLEDGE | STATUS_DRIVER)
            .unwrap();

        device
            .configure_queue(CONTROLQ, GuestAddress::new(0x1000), 256)
            .unwrap();

        assert!(device.queue_ready(CONTROLQ).unwrap());
        assert!(!device.queue_ready(CURSORQ).unwrap());
    }

    #[test]
    fn cursor_queue_can_be_configured() {
        let mut device = VirtioGpuDevice::new();

        device
            .set_status(STATUS_ACKNOWLEDGE | STATUS_DRIVER)
            .unwrap();

        device
            .configure_queue(CURSORQ, GuestAddress::new(0x4000), 64)
            .unwrap();

        assert!(device.queue_ready(CURSORQ).unwrap());
    }

    #[test]
    fn invalid_queue_is_rejected() {
        let mut device = VirtioGpuDevice::new();

        device
            .set_status(STATUS_ACKNOWLEDGE | STATUS_DRIVER)
            .unwrap();

        assert_eq!(
            device.configure_queue(2, GuestAddress::new(0x1000), 256),
            Err(DeviceError::QueueIndexInvalid)
        );
    }

    #[test]
    fn reset_clears_device_state() {
        let mut device = VirtioGpuDevice::new();

        device
            .set_status(STATUS_ACKNOWLEDGE | STATUS_DRIVER)
            .unwrap();

        device
            .set_driver_features((GpuFeatures::VIRGL | GpuFeatures::RESOURCE_BLOB).bits())
            .unwrap();

        device
            .configure_queue(CONTROLQ, GuestAddress::new(0x1000), 256)
            .unwrap();

        device.reset();

        assert_eq!(device.status(), DeviceStatus::empty());
        assert_eq!(device.driver_features(), GpuFeatures::empty());
        assert!(!device.queue_ready(CONTROLQ).unwrap());
    }

    #[test]
    fn get_display_info_is_processed() {
        let mut device = VirtioGpuDevice::new();

        device
            .set_status(STATUS_ACKNOWLEDGE | STATUS_DRIVER)
            .unwrap();

        device
            .set_driver_features(
                (GpuFeatures::VIRGL | GpuFeatures::RESOURCE_BLOB | GpuFeatures::CONTEXT_INIT)
                    .bits(),
            )
            .unwrap();

        device
            .set_status(STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_FEATURES_OK)
            .unwrap();

        device
            .create_resource_2d(ResourceCreate2D {
                header: CtrlHeader::new(CMD_RESOURCE_CREATE_2D),
                resource_id: 1,
                format: VirtioGpuFormat::B8G8R8A8Unorm,
                width: 1920,
                height: 1080,
            })
            .unwrap();

        device
            .set_scanout(ResourceSetScanout {
                scanout_id: 0,
                resource_id: 1,
                rect: [0, 0, 1920, 1080],
            })
            .unwrap();

        device
            .configure_queue(CONTROLQ, GuestAddress::new(0x1000), 8)
            .unwrap();

        device
            .set_status(STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_FEATURES_OK | STATUS_DRIVER_OK)
            .unwrap();

        let queue = device.control_queue_mut().unwrap();

        // Queue memory starts at 0x1000 and is 222 bytes for queue size 8.
        let request_addr = GuestAddress::new(0x14f0);
        let response_addr = GuestAddress::new(0x1500);

        let request = CtrlHeader::new(CMD_GET_DISPLAY_INFO);
        queue
            .memory_mut()
            .write(request_addr, &request.encode_le())
            .unwrap();

        let head = queue
            .add_chain(&[
                crate::virtio_gpu::transport::virtqueue::split::Descriptor {
                    addr: request_addr,
                    len: CtrlHeader::SIZE as u32,
                    flags: 0,
                    next: 0,
                },
                crate::virtio_gpu::transport::virtqueue::split::Descriptor {
                    addr: response_addr,
                    len: RespDisplayInfo::SIZE as u32,
                    flags: DESC_F_WRITE,
                    next: 0,
                },
            ])
            .unwrap();

        assert_eq!(head, 0);

        assert!(device.control_queue().is_some());

        device.process_queue().unwrap();

        let queue = device.control_queue_mut().unwrap();

        let mut response_bytes = vec![0u8; RespDisplayInfo::SIZE];

        queue
            .memory_mut()
            .read(response_addr, &mut response_bytes)
            .unwrap();

        println!("response len = {}", response_bytes.len());

        let header = CtrlHeader::decode_le(&response_bytes);
        println!("header = {:?}", header);

        let response = RespDisplayInfo::decode_le(&response_bytes).unwrap();

        assert_eq!(response.header.typ, RESP_OK_DISPLAY_INFO);
        assert_eq!(response.pmodes[0].rect.width, 1920);
        assert_eq!(response.pmodes[0].rect.height, 1080);
        assert_eq!(response.pmodes[0].enabled, 1);

        let used = queue.pop_used().unwrap().unwrap();

        assert_eq!(used.id, 0);
        assert_eq!(used.len, RespDisplayInfo::SIZE as u32);
    }

    #[test]
    fn create_resource_2d_allocates_resource() {
        let mut device = VirtioGpuDevice::new();

        let request = ResourceCreate2D {
            header: CtrlHeader::new(CMD_RESOURCE_CREATE_2D),

            resource_id: 1,

            format: VirtioGpuFormat::B8G8R8A8Unorm,

            width: 1980,
            height: 1080,
        };

        device.create_resource_2d(request).unwrap();

        let resource = device.resource.get(1).unwrap();

        assert_eq!(resource.width, 1980);
        assert_eq!(resource.height, 1080);

        assert_eq!(resource.data.len(), 1980 * 1080 * 4);
    }

    #[test]
    fn create_resource_returns_ok_nodata() {
        let response = RespOkNoData::new();

        assert_eq!(response.header.typ, RESP_OK_NODATA);
    }

    #[test]
    fn resource_flush_is_processed() {
        let mut device = VirtioGpuDevice::new();

        device
            .set_status(STATUS_ACKNOWLEDGE | STATUS_DRIVER)
            .unwrap();

        device
            .set_driver_features(
                (GpuFeatures::VIRGL | GpuFeatures::RESOURCE_BLOB | GpuFeatures::CONTEXT_INIT)
                    .bits(),
            )
            .unwrap();

        device
            .set_status(STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_FEATURES_OK)
            .unwrap();

        // Create the resource that will be flushed.
        device
            .create_resource_2d(ResourceCreate2D {
                header: CtrlHeader::new(CMD_RESOURCE_CREATE_2D),
                resource_id: 1,
                format: VirtioGpuFormat::B8G8R8A8Unorm,
                width: 1920,
                height: 1080,
            })
            .unwrap();

        device
            .configure_queue(CONTROLQ, GuestAddress::new(0x1000), 8)
            .unwrap();

        device
            .set_status(STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_FEATURES_OK | STATUS_DRIVER_OK)
            .unwrap();

        let queue = device.control_queue_mut().unwrap();

        let request_addr = GuestAddress::new(0x14f0);
        let response_addr = GuestAddress::new(0x1520);

        // CtrlHeader (24) + resource_id (4) + rect (16) = 44 bytes.
        let mut request_bytes = [0u8; ResourceFlush::SIZE];

        request_bytes[0..CtrlHeader::SIZE]
            .copy_from_slice(&CtrlHeader::new(CMD_RESOURCE_FLUSH).encode_le());

        request_bytes[24..28].copy_from_slice(&1u32.to_le_bytes());

        request_bytes[28..32].copy_from_slice(&0u32.to_le_bytes());
        request_bytes[32..36].copy_from_slice(&0u32.to_le_bytes());
        request_bytes[36..40].copy_from_slice(&1920u32.to_le_bytes());
        request_bytes[40..44].copy_from_slice(&1080u32.to_le_bytes());

        queue
            .memory_mut()
            .write(request_addr, &request_bytes)
            .unwrap();

        let head = queue
            .add_chain(&[
                crate::virtio_gpu::transport::virtqueue::split::Descriptor {
                    addr: request_addr,
                    len: ResourceFlush::SIZE as u32,
                    flags: 0,
                    next: 0,
                },
                crate::virtio_gpu::transport::virtqueue::split::Descriptor {
                    addr: response_addr,
                    len: RespOkNoData::SIZE as u32,
                    flags: DESC_F_WRITE,
                    next: 0,
                },
            ])
            .unwrap();

        assert_eq!(head, 0);

        device.process_queue().unwrap();

        let queue = device.control_queue_mut().unwrap();

        let mut response_bytes = vec![0u8; RespOkNoData::SIZE];

        queue
            .memory_mut()
            .read(response_addr, &mut response_bytes)
            .unwrap();

        let header = CtrlHeader::decode_le(&response_bytes).unwrap();

        assert_eq!(header.typ, RESP_OK_NODATA);

        let used = queue.pop_used().unwrap().unwrap();

        assert_eq!(used.id, 0);
        assert_eq!(used.len, RespOkNoData::SIZE as u32);
    }

    #[test]
    fn resource_attach_backing_is_processed() {
        let mut device = VirtioGpuDevice::new();

        device
            .set_status(STATUS_ACKNOWLEDGE | STATUS_DRIVER)
            .unwrap();

        device
            .set_driver_features(
                (GpuFeatures::VIRGL | GpuFeatures::RESOURCE_BLOB | GpuFeatures::CONTEXT_INIT)
                    .bits(),
            )
            .unwrap();

        device
            .set_status(STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_FEATURES_OK)
            .unwrap();

        // Create a 1920x1080 RGBA resource.
        device
            .create_resource_2d(ResourceCreate2D {
                header: CtrlHeader::new(CMD_RESOURCE_CREATE_2D),
                resource_id: 1,
                format: VirtioGpuFormat::B8G8R8A8Unorm,
                width: 1920,
                height: 1080,
            })
            .unwrap();

        device
            .configure_queue(CONTROLQ, GuestAddress::new(0x1000), 8)
            .unwrap();

        device
            .set_status(STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_FEATURES_OK | STATUS_DRIVER_OK)
            .unwrap();

        let request_addr = GuestAddress::new(0x14f0);
        let response_addr = GuestAddress::new(0x1520);

        // CtrlHeader (24)
        // + resource_id (4)
        // + nr_entries (4)
        // + one VirtioGpuMemEntry (16)
        // = 48 bytes.
        let mut request_bytes = [0u8; 48];

        request_bytes[0..CtrlHeader::SIZE]
            .copy_from_slice(&CtrlHeader::new(CMD_RESOURCE_ATTACH_BACKING).encode_le());

        // resource_id = 1
        request_bytes[24..28].copy_from_slice(&1u32.to_le_bytes());

        // nr_entries = 1
        request_bytes[28..32].copy_from_slice(&1u32.to_le_bytes());

        // Backing memory entry.
        let backing_addr = 0x200000u64;
        let backing_len = 1920u32 * 1080u32 * 4;

        request_bytes[32..40].copy_from_slice(&backing_addr.to_le_bytes());
        request_bytes[40..44].copy_from_slice(&backing_len.to_le_bytes());

        // padding = 0
        request_bytes[44..48].copy_from_slice(&0u32.to_le_bytes());

        {
            let queue = device.control_queue_mut().unwrap();

            queue
                .memory_mut()
                .write(request_addr, &request_bytes)
                .unwrap();

            let head = queue
                .add_chain(&[
                    crate::virtio_gpu::transport::virtqueue::split::Descriptor {
                        addr: request_addr,
                        len: request_bytes.len() as u32,
                        flags: 0,
                        next: 0,
                    },
                    crate::virtio_gpu::transport::virtqueue::split::Descriptor {
                        addr: response_addr,
                        len: RespOkNoData::SIZE as u32,
                        flags: DESC_F_WRITE,
                        next: 0,
                    },
                ])
                .unwrap();

            assert_eq!(head, 0);
        }

        // Process the command.
        device.process_queue().unwrap();

        // Verify the response.
        {
            let queue = device.control_queue_mut().unwrap();

            let mut response_bytes = vec![0u8; RespOkNoData::SIZE];

            queue
                .memory_mut()
                .read(response_addr, &mut response_bytes)
                .unwrap();

            let header = CtrlHeader::decode_le(&response_bytes).unwrap();

            assert_eq!(header.typ, RESP_OK_NODATA);
        }

        // Verify the resource now has the backing entry.
        {
            let resource = device.resource.get(1).unwrap();

            assert_eq!(resource.backing.len(), 1);
            assert_eq!(resource.backing[0].addr, backing_addr);
            assert_eq!(resource.backing[0].length, backing_len);
        }

        // Verify Used Ring.
        {
            let queue = device.control_queue_mut().unwrap();

            let used = queue.pop_used().unwrap().unwrap();

            assert_eq!(used.id, 0);
            assert_eq!(used.len, RespOkNoData::SIZE as u32);
        }
    }
    #[test]
    fn transfer_to_host_2d_is_processed() {
        let mut device = VirtioGpuDevice::new();

        device
            .set_status(STATUS_ACKNOWLEDGE | STATUS_DRIVER)
            .unwrap();

        device
            .set_driver_features(
                (GpuFeatures::VIRGL | GpuFeatures::RESOURCE_BLOB | GpuFeatures::CONTEXT_INIT)
                    .bits(),
            )
            .unwrap();

        device
            .set_status(STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_FEATURES_OK)
            .unwrap();

        // Small resource: 4x4 RGBA = 64 bytes
        device
            .create_resource_2d(ResourceCreate2D {
                header: CtrlHeader::new(CMD_RESOURCE_CREATE_2D),
                resource_id: 1,
                format: VirtioGpuFormat::B8G8R8A8Unorm,
                width: 4,
                height: 4,
            })
            .unwrap();

        device
            .configure_queue(CONTROLQ, GuestAddress::new(0x1000), 8)
            .unwrap();

        device
            .set_status(STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_FEATURES_OK | STATUS_DRIVER_OK)
            .unwrap();

        let queue = device.control_queue_mut().unwrap();

        let request_addr = GuestAddress::new(0x14f0);
        let response_addr = GuestAddress::new(0x1540);
        let backing_addr = GuestAddress::new(0x200000);

        // ---------------------------------------------------------
        // Put known framebuffer data into Guest Memory.
        // ---------------------------------------------------------

        let backing_data: Vec<u8> = (0u8..64u8).collect();

        queue
            .memory_mut()
            .write(backing_addr, &backing_data)
            .unwrap();

        // ---------------------------------------------------------
        // Attach backing.
        // ---------------------------------------------------------

        let mut attach = [0u8; 48];

        attach[0..CtrlHeader::SIZE]
            .copy_from_slice(&CtrlHeader::new(CMD_RESOURCE_ATTACH_BACKING).encode_le());

        attach[24..28].copy_from_slice(&1u32.to_le_bytes());
        attach[28..32].copy_from_slice(&1u32.to_le_bytes());

        attach[32..40].copy_from_slice(&backing_addr.0.to_le_bytes());
        attach[40..44].copy_from_slice(&(64u32).to_le_bytes());
        attach[44..48].copy_from_slice(&0u32.to_le_bytes());

        queue.memory_mut().write(request_addr, &attach).unwrap();

        queue
            .add_chain(&[
                crate::virtio_gpu::transport::virtqueue::split::Descriptor {
                    addr: request_addr,
                    len: attach.len() as u32,
                    flags: 0,
                    next: 0,
                },
                crate::virtio_gpu::transport::virtqueue::split::Descriptor {
                    addr: response_addr,
                    len: RespOkNoData::SIZE as u32,
                    flags: DESC_F_WRITE,
                    next: 0,
                },
            ])
            .unwrap();

        device.process_queue().unwrap();

        // ---------------------------------------------------------
        // Verify attach response.
        // ---------------------------------------------------------

        let queue = device.control_queue_mut().unwrap();

        let mut response = vec![0u8; RespOkNoData::SIZE];

        queue
            .memory_mut()
            .read(response_addr, &mut response)
            .unwrap();

        let header = CtrlHeader::decode_le(&response).unwrap();

        assert_eq!(header.typ, RESP_OK_NODATA);

        let _ = (queue);

        // ---------------------------------------------------------
        // Build TRANSFER_TO_HOST_2D request.
        // ---------------------------------------------------------

        let request_addr = GuestAddress::new(0x1600);
        let response_addr = GuestAddress::new(0x1640);

        let mut transfer = [0u8; 56];

        transfer[0..CtrlHeader::SIZE]
            .copy_from_slice(&CtrlHeader::new(CMD_TRANSFER_TO_HOST_2D).encode_le());

        // resource_id
        transfer[24..28].copy_from_slice(&1u32.to_le_bytes());

        // rect.x
        transfer[28..32].copy_from_slice(&0u32.to_le_bytes());

        // rect.y
        transfer[32..36].copy_from_slice(&0u32.to_le_bytes());

        // rect.width
        transfer[36..40].copy_from_slice(&4u32.to_le_bytes());

        // rect.height
        transfer[40..44].copy_from_slice(&4u32.to_le_bytes());

        // padding / remaining bytes
        transfer[44..56].fill(0);

        let queue = device.control_queue_mut().unwrap();

        queue.memory_mut().write(request_addr, &transfer).unwrap();

        queue
            .add_chain(&[
                crate::virtio_gpu::transport::virtqueue::split::Descriptor {
                    addr: request_addr,
                    len: transfer.len() as u32,
                    flags: 0,
                    next: 0,
                },
                crate::virtio_gpu::transport::virtqueue::split::Descriptor {
                    addr: response_addr,
                    len: RespOkNoData::SIZE as u32,
                    flags: DESC_F_WRITE,
                    next: 0,
                },
            ])
            .unwrap();

        device.process_queue().unwrap();

        // ---------------------------------------------------------
        // Verify response.
        // ---------------------------------------------------------

        let queue = device.control_queue_mut().unwrap();

        let mut response = vec![0u8; RespOkNoData::SIZE];

        queue
            .memory_mut()
            .read(response_addr, &mut response)
            .unwrap();

        let header = CtrlHeader::decode_le(&response).unwrap();

        assert_eq!(header.typ, RESP_OK_NODATA);

        let _ = (queue);

        // ---------------------------------------------------------
        // Verify resource contents.
        // ---------------------------------------------------------

        let resource = device.resource.get(1).unwrap();

        assert_eq!(resource.data.len(), 64);
        assert_eq!(resource.data, backing_data);

        // ---------------------------------------------------------
        // Verify Used Ring.
        // ---------------------------------------------------------

        let queue = device.control_queue_mut().unwrap();

        let used = queue.pop_used().unwrap().unwrap();

        assert_eq!(used.id, 0);
        assert_eq!(used.len, RespOkNoData::SIZE as u32);
    }

    #[test]
    fn resource_flush_marks_dirty_region() {
        let mut device = VirtioGpuDevice::new();

        device
            .create_resource_2d(ResourceCreate2D {
                header: CtrlHeader::new(CMD_RESOURCE_CREATE_2D),
                resource_id: 1,
                format: VirtioGpuFormat::B8G8R8A8Unorm,
                width: 800,
                height: 600,
            })
            .unwrap();

        let req = ResourceFlush {
            resource_id: 1,
            rect: [10, 20, 300, 200],
        };

        device.resource_flush(req).unwrap();

        let resource = device.resource.get(1).unwrap();

        assert_eq!(resource.dirty, Some([10, 20, 300, 200]));
    }

    #[test]
    fn framebuffer_returns_scanout_pixels() {
        let mut device = VirtioGpuDevice::new();

        device
            .create_resource_2d(ResourceCreate2D {
                header: CtrlHeader::new(CMD_RESOURCE_CREATE_2D),
                resource_id: 1,
                format: VirtioGpuFormat::B8G8R8A8Unorm,
                width: 64,
                height: 64,
            })
            .unwrap();

        device
            .set_scanout(ResourceSetScanout {
                scanout_id: 0,
                resource_id: 1,
                rect: [0, 0, 64, 64],
            })
            .unwrap();

        let fb = device.framebuffer(0).unwrap();

        assert_eq!(fb.len(), 64 * 64 * 4);
    }

    #[test]
    fn full_display_pipeline_test() {
        let mut device = VirtioGpuDevice::new();

        device
            .create_resource_2d(ResourceCreate2D {
                header: CtrlHeader::new(CMD_RESOURCE_CREATE_2D),
                resource_id: 1,
                format: VirtioGpuFormat::B8G8R8A8Unorm,
                width: 800,
                height: 600,
            })
            .unwrap();

        // ساخت تصویر قرمز در Guest Memory
        let mut image = vec![0u8; 800 * 600 * 4];

        for pixel in image.chunks_exact_mut(4) {
            pixel[0] = 0; // B
            pixel[1] = 0; // G
            pixel[2] = 255; // R
            pixel[3] = 255; // A
        }

        // نوشتن در RAM مهمان
        device
            .memory()
            .write(GuestAddress::new(0x200000), &image)
            .unwrap();

        // attach backing
        {
            let resource = device.resource.get_mut(1).unwrap();

            resource.backing.push(VirtioGpuMemEntry {
                addr: 0x200000,
                length: (800 * 600 * 4) as u32,
                padding: 0,
            });
        }

        // انتقال از Guest Memory به Resource
        device
            .transfer_to_host(ResourceTransferToHost2D {
                resource_id: 1,
                offset: 0,
                rect: Rect {
                    x: 0,
                    y: 0,
                    width: 1920,
                    height: 1080,
                },
            })
            .unwrap();

        // اتصال resource به scanout
        device
            .set_scanout(ResourceSetScanout {
                scanout_id: 0,
                resource_id: 1,
                rect: [0, 0, 1920, 1080],
            })
            .unwrap();

        // نمایش فریم
        device
            .resource_flush(ResourceFlush {
                resource_id: 1,
                rect: [0, 0, 1920, 1080],
            })
            .unwrap();

        device.present();

        let renderer = device.renderer.as_ref().unwrap();

        let fb = renderer.framebuffer();

        assert_eq!(fb.width, 800);
        assert_eq!(fb.height, 600);

        // اولین پیکسل باید قرمز باشد
        assert_eq!(fb.data[0], 0); // B
        assert_eq!(fb.data[1], 0); // G
        assert_eq!(fb.data[2], 255); // R
        assert_eq!(fb.data[3], 255); // A

        device.render_frame();
    }
}
