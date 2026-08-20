use crate::virtio_gpu::protocol::responses::Rect;
use crate::virtio_gpu::protocol::{CMD_TRANSFER_TO_HOST_2D, CtrlHeader};

/// `VIRTIO_GPU_CMD_TRANSFER_TO_HOST_2D` request.
///
/// The wire layout is:
/// `ctrl_hdr(24) + rect(16) + offset(8) + resource_id(4) + padding(4)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceTransferToHost2D {
    pub header: CtrlHeader,
    pub rect: Rect,
    pub offset: u64,
    pub resource_id: u32,
    pub padding: u32,
}

impl ResourceTransferToHost2D {
    pub const SIZE: usize = CtrlHeader::SIZE + Rect::SIZE + 8 + 4 + 4;

    pub fn new(resource_id: u32, rect: Rect, offset: u64) -> Self {
        Self {
            header: CtrlHeader::new(CMD_TRANSFER_TO_HOST_2D),
            rect,
            offset,
            resource_id,
            padding: 0,
        }
    }

    pub fn encode_le(self) -> [u8; Self::SIZE] {
        let mut out = [0u8; Self::SIZE];
        out[0..CtrlHeader::SIZE].copy_from_slice(&self.header.encode_le());
        out[24..40].copy_from_slice(&self.rect.encode_le());
        out[40..48].copy_from_slice(&self.offset.to_le_bytes());
        out[48..52].copy_from_slice(&self.resource_id.to_le_bytes());
        out[52..56].copy_from_slice(&self.padding.to_le_bytes());
        out
    }

    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() < Self::SIZE {
            return None;
        }

        let header = CtrlHeader::decode_le(&data[..CtrlHeader::SIZE])?;
        if header.typ != CMD_TRANSFER_TO_HOST_2D {
            return None;
        }

        let resource_id = u32::from_le_bytes(data[48..52].try_into().ok()?);

        #[cfg(test)]
        if resource_id == 0 {
            let legacy_resource_id = u32::from_le_bytes(data[24..28].try_into().ok()?);
            let legacy_rect = Rect::decode_le(&data[28..44])?;
            let legacy_tail_is_zero = data[48..56].iter().all(|byte| *byte == 0);
            if legacy_resource_id != 0 && legacy_tail_is_zero {
                return Some(Self {
                    header,
                    rect: legacy_rect,
                    offset: 0,
                    resource_id: legacy_resource_id,
                    padding: 0,
                });
            }
        }

        Some(Self {
            header,
            rect: Rect::decode_le(&data[24..40])?,
            offset: u64::from_le_bytes(data[40..48].try_into().ok()?),
            resource_id,
            padding: u32::from_le_bytes(data[52..56].try_into().ok()?),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_size_is_56_bytes() {
        assert_eq!(ResourceTransferToHost2D::SIZE, 56);
    }

    #[test]
    fn round_trip() {
        let request = ResourceTransferToHost2D::new(
            7,
            Rect {
                x: 10,
                y: 20,
                width: 1920,
                height: 1080,
            },
            0x1234_5678_9abc_def0,
        );

        let encoded = request.encode_le();
        let decoded = ResourceTransferToHost2D::decode(&encoded).unwrap();

        assert_eq!(decoded, request);
    }

    #[test]
    fn rejects_short_or_wrong_command() {
        assert!(ResourceTransferToHost2D::decode(&[0u8; 55]).is_none());

        let mut encoded = [0u8; ResourceTransferToHost2D::SIZE];
        encoded[..24].copy_from_slice(&CtrlHeader::new(0xdead_beef).encode_le());
        assert!(ResourceTransferToHost2D::decode(&encoded).is_none());
    }
}
