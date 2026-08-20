use crate::virtio_gpu::device::DeviceError;
use crate::virtio_gpu::protocol::{formats::VirtioGpuFormat, header::CtrlHeader};

#[derive(Debug, Clone, Copy)]
pub struct ResourceCreate2D {
    pub header: CtrlHeader,
    pub resource_id: u32,
    pub format: VirtioGpuFormat,
    pub width: u32,
    pub height: u32,
}

impl ResourceCreate2D {
    pub const SIZE: usize = 40;

    pub fn decode(data: &[u8]) -> Result<Self, DeviceError> {
        if data.len() < Self::SIZE {
            return Err(DeviceError::InvalidRequest);
        }

        let header = CtrlHeader::decode_le(&data[0..24]).ok_or(DeviceError::InvalidRequest)?;

        let resource_id = u32::from_le_bytes(data[24..28].try_into().unwrap());

        let format_raw = u32::from_le_bytes(data[28..32].try_into().unwrap());

        let format = VirtioGpuFormat::from_u32(format_raw).ok_or(DeviceError::InvalidFormat)?;

        let width = u32::from_le_bytes(data[32..36].try_into().unwrap());

        let height = u32::from_le_bytes(data[36..40].try_into().unwrap());

        Ok(Self {
            header,
            resource_id,
            format,
            width,
            height,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ResourceAttachBacking {
    pub resource_id: u32,
    pub nr_entries: u32,
}

impl ResourceAttachBacking {
    pub const SIZE: usize = 32;

    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() < Self::SIZE {
            return None;
        }

        Some(Self {
            resource_id: u32::from_le_bytes(data[24..28].try_into().ok()?),
            nr_entries: u32::from_le_bytes(data[28..32].try_into().ok()?),
        })
    }
}
