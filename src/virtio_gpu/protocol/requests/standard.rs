use crate::virtio_gpu::protocol::commands::*;
use crate::virtio_gpu::protocol::responses::Rect;
use crate::virtio_gpu::protocol::CtrlHeader;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceUnref {
    pub header: CtrlHeader,
    pub resource_id: u32,
    pub padding: u32,
}

impl ResourceUnref {
    pub const SIZE: usize = 32;

    pub fn new(resource_id: u32) -> Self {
        Self { header: CtrlHeader::new(CMD_RESOURCE_UNREF), resource_id, padding: 0 }
    }

    pub fn encode_le(self) -> [u8; Self::SIZE] {
        let mut out = [0; Self::SIZE];
        out[..24].copy_from_slice(&self.header.encode_le());
        out[24..28].copy_from_slice(&self.resource_id.to_le_bytes());
        out[28..32].copy_from_slice(&self.padding.to_le_bytes());
        out
    }

    pub fn decode_le(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < Self::SIZE { return None; }
        let header = CtrlHeader::decode_le(&bytes[..24])?;
        if header.typ != CMD_RESOURCE_UNREF { return None; }
        Some(Self {
            header,
            resource_id: u32::from_le_bytes(bytes[24..28].try_into().ok()?),
            padding: u32::from_le_bytes(bytes[28..32].try_into().ok()?),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceDetachBacking {
    pub header: CtrlHeader,
    pub resource_id: u32,
    pub padding: u32,
}

impl ResourceDetachBacking {
    pub const SIZE: usize = 32;

    pub fn new(resource_id: u32) -> Self {
        Self { header: CtrlHeader::new(CMD_RESOURCE_DETACH_BACKING), resource_id, padding: 0 }
    }

    pub fn encode_le(self) -> [u8; Self::SIZE] {
        let mut out = [0; Self::SIZE];
        out[..24].copy_from_slice(&self.header.encode_le());
        out[24..28].copy_from_slice(&self.resource_id.to_le_bytes());
        out[28..32].copy_from_slice(&self.padding.to_le_bytes());
        out
    }

    pub fn decode_le(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < Self::SIZE { return None; }
        let header = CtrlHeader::decode_le(&bytes[..24])?;
        if header.typ != CMD_RESOURCE_DETACH_BACKING { return None; }
        Some(Self {
            header,
            resource_id: u32::from_le_bytes(bytes[24..28].try_into().ok()?),
            padding: u32::from_le_bytes(bytes[28..32].try_into().ok()?),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GetEdid {
    pub header: CtrlHeader,
    pub scanout: u32,
    pub padding: u32,
}

impl GetEdid {
    pub const SIZE: usize = 32;

    pub fn new(scanout: u32) -> Self {
        Self { header: CtrlHeader::new(CMD_GET_EDID), scanout, padding: 0 }
    }

    pub fn encode_le(self) -> [u8; Self::SIZE] {
        let mut out = [0; Self::SIZE];
        out[..24].copy_from_slice(&self.header.encode_le());
        out[24..28].copy_from_slice(&self.scanout.to_le_bytes());
        out[28..32].copy_from_slice(&self.padding.to_le_bytes());
        out
    }

    pub fn decode_le(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < Self::SIZE { return None; }
        let header = CtrlHeader::decode_le(&bytes[..24])?;
        if header.typ != CMD_GET_EDID { return None; }
        Some(Self {
            header,
            scanout: u32::from_le_bytes(bytes[24..28].try_into().ok()?),
            padding: u32::from_le_bytes(bytes[28..32].try_into().ok()?),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceAssignUuid {
    pub header: CtrlHeader,
    pub resource_id: u32,
    pub padding: u32,
}

impl ResourceAssignUuid {
    pub const SIZE: usize = 32;

    pub fn new(resource_id: u32) -> Self {
        Self { header: CtrlHeader::new(CMD_RESOURCE_ASSIGN_UUID), resource_id, padding: 0 }
    }

    pub fn encode_le(self) -> [u8; Self::SIZE] {
        let mut out = [0; Self::SIZE];
        out[..24].copy_from_slice(&self.header.encode_le());
        out[24..28].copy_from_slice(&self.resource_id.to_le_bytes());
        out[28..32].copy_from_slice(&self.padding.to_le_bytes());
        out
    }

    pub fn decode_le(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < Self::SIZE { return None; }
        let header = CtrlHeader::decode_le(&bytes[..24])?;
        if header.typ != CMD_RESOURCE_ASSIGN_UUID { return None; }
        Some(Self {
            header,
            resource_id: u32::from_le_bytes(bytes[24..28].try_into().ok()?),
            padding: u32::from_le_bytes(bytes[28..32].try_into().ok()?),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SetScanoutBlob {
    pub header: CtrlHeader,
    pub rect: Rect,
    pub scanout_id: u32,
    pub resource_id: u32,
    pub width: u32,
    pub height: u32,
    pub format: u32,
    pub padding: u32,
    pub strides: [u32; 4],
    pub offsets: [u32; 4],
}

impl SetScanoutBlob {
    pub const SIZE: usize = 96;

    pub fn new(
        scanout_id: u32,
        resource_id: u32,
        rect: Rect,
        width: u32,
        height: u32,
        format: u32,
        strides: [u32; 4],
        offsets: [u32; 4],
    ) -> Self {
        Self {
            header: CtrlHeader::new(CMD_SET_SCANOUT_BLOB),
            rect,
            scanout_id,
            resource_id,
            width,
            height,
            format,
            padding: 0,
            strides,
            offsets,
        }
    }

    pub fn encode_le(self) -> [u8; Self::SIZE] {
        let mut out = [0; Self::SIZE];
        out[..24].copy_from_slice(&self.header.encode_le());
        out[24..40].copy_from_slice(&self.rect.encode_le());
        out[40..44].copy_from_slice(&self.scanout_id.to_le_bytes());
        out[44..48].copy_from_slice(&self.resource_id.to_le_bytes());
        out[48..52].copy_from_slice(&self.width.to_le_bytes());
        out[52..56].copy_from_slice(&self.height.to_le_bytes());
        out[56..60].copy_from_slice(&self.format.to_le_bytes());
        out[60..64].copy_from_slice(&self.padding.to_le_bytes());
        for (i, value) in self.strides.iter().enumerate() {
            let start = 64 + i * 4;
            out[start..start + 4].copy_from_slice(&value.to_le_bytes());
        }
        for (i, value) in self.offsets.iter().enumerate() {
            let start = 80 + i * 4;
            out[start..start + 4].copy_from_slice(&value.to_le_bytes());
        }
        out
    }

    pub fn decode_le(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < Self::SIZE { return None; }
        let header = CtrlHeader::decode_le(&bytes[..24])?;
        if header.typ != CMD_SET_SCANOUT_BLOB { return None; }
        let mut strides = [0; 4];
        let mut offsets = [0; 4];
        for i in 0..4 {
            let s = 64 + i * 4;
            strides[i] = u32::from_le_bytes(bytes[s..s + 4].try_into().ok()?);
            let o = 80 + i * 4;
            offsets[i] = u32::from_le_bytes(bytes[o..o + 4].try_into().ok()?);
        }
        Some(Self {
            header,
            rect: Rect::decode_le(&bytes[24..40])?,
            scanout_id: u32::from_le_bytes(bytes[40..44].try_into().ok()?),
            resource_id: u32::from_le_bytes(bytes[44..48].try_into().ok()?),
            width: u32::from_le_bytes(bytes[48..52].try_into().ok()?),
            height: u32::from_le_bytes(bytes[52..56].try_into().ok()?),
            format: u32::from_le_bytes(bytes[56..60].try_into().ok()?),
            padding: u32::from_le_bytes(bytes[60..64].try_into().ok()?),
            strides,
            offsets,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceCreate3D {
    pub header: CtrlHeader,
    pub resource_id: u32,
    pub target: u32,
    pub format: u32,
    pub bind: u32,
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub array_size: u32,
    pub last_level: u32,
    pub nr_samples: u32,
    pub flags: u32,
    pub padding: u32,
}

impl ResourceCreate3D {
    pub const SIZE: usize = 72;

    pub fn encode_le(self) -> [u8; Self::SIZE] {
        let mut out = [0; Self::SIZE];
        out[..24].copy_from_slice(&self.header.encode_le());
        let values = [
            self.resource_id, self.target, self.format, self.bind, self.width, self.height,
            self.depth, self.array_size, self.last_level, self.nr_samples, self.flags, self.padding,
        ];
        for (i, value) in values.iter().enumerate() {
            let s = 24 + i * 4;
            out[s..s + 4].copy_from_slice(&value.to_le_bytes());
        }
        out
    }

    pub fn decode_le(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < Self::SIZE { return None; }
        let header = CtrlHeader::decode_le(&bytes[..24])?;
        if header.typ != CMD_RESOURCE_CREATE_3D { return None; }
        let read = |n: usize| -> Option<u32> {
            let s = 24 + n * 4;
            Some(u32::from_le_bytes(bytes[s..s + 4].try_into().ok()?))
        };
        Some(Self {
            header,
            resource_id: read(0)?,
            target: read(1)?,
            format: read(2)?,
            bind: read(3)?,
            width: read(4)?,
            height: read(5)?,
            depth: read(6)?,
            array_size: read(7)?,
            last_level: read(8)?,
            nr_samples: read(9)?,
            flags: read(10)?,
            padding: read(11)?,
        })
    }
}

pub const RESOURCE_FLAG_Y_0_TOP: u32 = 1 << 0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Box3D {
    pub x: u32,
    pub y: u32,
    pub z: u32,
    pub w: u32,
    pub h: u32,
    pub d: u32,
}

impl Box3D {
    pub const SIZE: usize = 24;

    pub fn encode_le(self) -> [u8; Self::SIZE] {
        let values = [self.x, self.y, self.z, self.w, self.h, self.d];
        let mut out = [0; Self::SIZE];
        for (i, value) in values.iter().enumerate() {
            out[i * 4..i * 4 + 4].copy_from_slice(&value.to_le_bytes());
        }
        out
    }

    pub fn decode_le(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < Self::SIZE { return None; }
        let mut values = [0u32; 6];
        for i in 0..6 {
            values[i] = u32::from_le_bytes(bytes[i * 4..i * 4 + 4].try_into().ok()?);
        }
        Some(Self { x: values[0], y: values[1], z: values[2], w: values[3], h: values[4], d: values[5] })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TransferHost3D {
    pub header: CtrlHeader,
    pub box_: Box3D,
    pub offset: u64,
    pub resource_id: u32,
    pub level: u32,
    pub stride: u32,
    pub layer_stride: u32,
}

impl TransferHost3D {
    pub const SIZE: usize = 72;

    pub fn encode_le(self) -> [u8; Self::SIZE] {
        let mut out = [0; Self::SIZE];
        out[..24].copy_from_slice(&self.header.encode_le());
        out[24..48].copy_from_slice(&self.box_.encode_le());
        out[48..56].copy_from_slice(&self.offset.to_le_bytes());
        out[56..60].copy_from_slice(&self.resource_id.to_le_bytes());
        out[60..64].copy_from_slice(&self.level.to_le_bytes());
        out[64..68].copy_from_slice(&self.stride.to_le_bytes());
        out[68..72].copy_from_slice(&self.layer_stride.to_le_bytes());
        out
    }

    pub fn decode_le(bytes: &[u8], expected_type: u32) -> Option<Self> {
        if bytes.len() < Self::SIZE { return None; }
        let header = CtrlHeader::decode_le(&bytes[..24])?;
        if header.typ != expected_type { return None; }
        Some(Self {
            header,
            box_: Box3D::decode_le(&bytes[24..48])?,
            offset: u64::from_le_bytes(bytes[48..56].try_into().ok()?),
            resource_id: u32::from_le_bytes(bytes[56..60].try_into().ok()?),
            level: u32::from_le_bytes(bytes[60..64].try_into().ok()?),
            stride: u32::from_le_bytes(bytes[64..68].try_into().ok()?),
            layer_stride: u32::from_le_bytes(bytes[68..72].try_into().ok()?),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct CursorPos {
    pub scanout_id: u32,
    pub x: u32,
    pub y: u32,
    pub padding: u32,
}

impl CursorPos {
    pub const SIZE: usize = 16;
    pub fn encode_le(self) -> [u8; Self::SIZE] {
        let values = [self.scanout_id, self.x, self.y, self.padding];
        let mut out = [0; Self::SIZE];
        for (i, value) in values.iter().enumerate() {
            out[i * 4..i * 4 + 4].copy_from_slice(&value.to_le_bytes());
        }
        out
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UpdateCursor {
    pub header: CtrlHeader,
    pub pos: CursorPos,
    pub resource_id: u32,
    pub hot_x: u32,
    pub hot_y: u32,
    pub padding: u32,
}

impl UpdateCursor {
    pub const SIZE: usize = 56;
    pub fn new(resource_id: u32, pos: CursorPos, hot_x: u32, hot_y: u32) -> Self {
        Self { header: CtrlHeader::new(CMD_UPDATE_CURSOR), pos, resource_id, hot_x, hot_y, padding: 0 }
    }

    pub fn encode_le(self) -> [u8; Self::SIZE] {
        let mut out = [0; Self::SIZE];
        out[..24].copy_from_slice(&self.header.encode_le());
        out[24..40].copy_from_slice(&self.pos.encode_le());
        out[40..44].copy_from_slice(&self.resource_id.to_le_bytes());
        out[44..48].copy_from_slice(&self.hot_x.to_le_bytes());
        out[48..52].copy_from_slice(&self.hot_y.to_le_bytes());
        out[52..56].copy_from_slice(&self.padding.to_le_bytes());
        out
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MoveCursor {
    pub header: CtrlHeader,
    pub pos: CursorPos,
}

impl MoveCursor {
    pub const SIZE: usize = 40;
    pub fn new(pos: CursorPos) -> Self { Self { header: CtrlHeader::new(CMD_MOVE_CURSOR), pos } }
    pub fn encode_le(self) -> [u8; Self::SIZE] {
        let mut out = [0; Self::SIZE];
        out[..24].copy_from_slice(&self.header.encode_le());
        out[24..40].copy_from_slice(&self.pos.encode_le());
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_sizes_match_linux_uapi() {
        assert_eq!(ResourceUnref::SIZE, 32);
        assert_eq!(ResourceDetachBacking::SIZE, 32);
        assert_eq!(GetEdid::SIZE, 32);
        assert_eq!(ResourceAssignUuid::SIZE, 32);
        assert_eq!(SetScanoutBlob::SIZE, 96);
        assert_eq!(ResourceCreate3D::SIZE, 72);
        assert_eq!(Box3D::SIZE, 24);
        assert_eq!(TransferHost3D::SIZE, 72);
        assert_eq!(CursorPos::SIZE, 16);
        assert_eq!(UpdateCursor::SIZE, 56);
        assert_eq!(MoveCursor::SIZE, 40);
    }

    #[test]
    fn scanout_blob_round_trip() {
        let request = SetScanoutBlob::new(
            1,
            2,
            Rect { x: 0, y: 0, width: 1920, height: 1080 },
            1920,
            1080,
            1,
            [7680, 0, 0, 0],
            [0, 0, 0, 0],
        );
        let bytes = request.encode_le();
        let decoded = SetScanoutBlob::decode_le(&bytes).unwrap();
        assert_eq!(decoded, request);
    }

    #[test]
    fn create_3d_round_trip() {
        let request = ResourceCreate3D {
            header: CtrlHeader::new(CMD_RESOURCE_CREATE_3D),
            resource_id: 7,
            target: 2,
            format: 1,
            bind: 0x10,
            width: 64,
            height: 64,
            depth: 1,
            array_size: 1,
            last_level: 0,
            nr_samples: 1,
            flags: RESOURCE_FLAG_Y_0_TOP,
            padding: 0,
        };
        let bytes = request.encode_le();
        assert_eq!(ResourceCreate3D::decode_le(&bytes), Some(request));
    }

    #[test]
    fn transfer_3d_round_trip() {
        let request = TransferHost3D {
            header: CtrlHeader::new(CMD_TRANSFER_TO_HOST_3D),
            box_: Box3D { x: 1, y: 2, z: 3, w: 4, h: 5, d: 6 },
            offset: 128,
            resource_id: 9,
            level: 2,
            stride: 256,
            layer_stride: 16384,
        };
        let bytes = request.encode_le();
        assert_eq!(
            TransferHost3D::decode_le(&bytes, CMD_TRANSFER_TO_HOST_3D),
            Some(request)
        );
    }
}
