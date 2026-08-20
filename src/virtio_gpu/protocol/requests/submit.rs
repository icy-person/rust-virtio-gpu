use crate::virtio_gpu::protocol::{CMD_SUBMIT_3D, CtrlHeader};

/// `VIRTIO_GPU_CMD_SUBMIT_3D` fixed request header.
///
/// The header is followed by `num_in_fences` little-endian u64 fence IDs,
/// then by `size` bytes of four-byte-aligned command data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Submit3D {
    pub header: CtrlHeader,
    pub size: u32,
    pub num_in_fences: u32,
}

impl Submit3D {
    pub const SIZE: usize = 32;

    pub fn new(ctx_id: u32, size: u32) -> Self {
        Self::with_in_fences(ctx_id, size, 0)
    }

    pub fn with_in_fences(ctx_id: u32, size: u32, num_in_fences: u32) -> Self {
        Self {
            header: CtrlHeader::new(CMD_SUBMIT_3D).with_context(ctx_id),
            size,
            num_in_fences,
        }
    }

    pub fn encode_le(self) -> [u8; Self::SIZE] {
        let mut out = [0u8; Self::SIZE];

        out[0..24].copy_from_slice(&self.header.encode_le());
        out[24..28].copy_from_slice(&self.size.to_le_bytes());
        out[28..32].copy_from_slice(&self.num_in_fences.to_le_bytes());

        out
    }

    pub fn decode_le(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < Self::SIZE {
            return None;
        }

        let header = CtrlHeader::decode_le(&bytes[0..24])?;
        if header.typ != CMD_SUBMIT_3D {
            return None;
        }

        Some(Self {
            header,
            size: u32::from_le_bytes(bytes[24..28].try_into().ok()?),
            num_in_fences: u32::from_le_bytes(bytes[28..32].try_into().ok()?),
        })
    }
}

/// A transport-independent SUBMIT_3D request including optional in-fence IDs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Submit3DCommand {
    pub request: Submit3D,
    pub in_fences: Vec<u64>,
    pub command_stream: Vec<u8>,
}

impl Submit3DCommand {
    pub fn new(ctx_id: u32, command_stream: Vec<u8>) -> Self {
        Self::with_in_fences(ctx_id, Vec::new(), command_stream)
    }

    pub fn with_in_fences(ctx_id: u32, in_fences: Vec<u64>, command_stream: Vec<u8>) -> Self {
        assert!(
            command_stream.len() <= u32::MAX as usize,
            "command stream is too large"
        );
        assert!(in_fences.len() <= u32::MAX as usize, "too many in-fences");
        assert_eq!(
            command_stream.len() % 4,
            0,
            "command stream must be 4-byte aligned"
        );

        let request =
            Submit3D::with_in_fences(ctx_id, command_stream.len() as u32, in_fences.len() as u32);

        Self {
            request,
            in_fences,
            command_stream,
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(
            Submit3D::SIZE + self.in_fences.len() * 8 + self.command_stream.len(),
        );

        out.extend_from_slice(&self.request.encode_le());
        for fence in &self.in_fences {
            out.extend_from_slice(&fence.to_le_bytes());
        }
        out.extend_from_slice(&self.command_stream);
        out
    }

    pub fn command_stream_size(&self) -> usize {
        self.command_stream.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn submit_size_matches_protocol() {
        assert_eq!(Submit3D::SIZE, 32);
    }

    #[test]
    fn submit_round_trip() {
        let request = Submit3D::new(42, 128);

        let bytes = request.encode_le();
        let decoded = Submit3D::decode_le(&bytes).unwrap();

        assert_eq!(decoded, request);
        assert_eq!(decoded.header.typ, CMD_SUBMIT_3D);
        assert_eq!(decoded.header.ctx_id, 42);
        assert_eq!(decoded.size, 128);
        assert_eq!(decoded.num_in_fences, 0);
    }

    #[test]
    fn submit_with_in_fences_round_trip() {
        let request = Submit3D::with_in_fences(7, 256, 3);
        let decoded = Submit3D::decode_le(&request.encode_le()).unwrap();
        assert_eq!(decoded, request);
        assert_eq!(decoded.num_in_fences, 3);
    }

    #[test]
    fn submit_rejects_short_or_wrong_command() {
        assert_eq!(Submit3D::decode_le(&[0u8; 31]), None);

        let mut bytes = [0u8; Submit3D::SIZE];
        bytes[..24].copy_from_slice(&CtrlHeader::new(0xdead_beef).encode_le());
        assert_eq!(Submit3D::decode_le(&bytes), None);
    }

    #[test]
    fn submit_command_stream_size_is_recorded() {
        let stream = vec![0x11, 0x22, 0x33, 0x44];
        let request = Submit3DCommand::new(7, stream.clone());

        assert_eq!(request.request.header.ctx_id, 7);
        assert_eq!(request.request.header.typ, CMD_SUBMIT_3D);
        assert_eq!(request.request.size, 4);
        assert_eq!(request.request.num_in_fences, 0);
        assert_eq!(request.command_stream_size(), 4);
        assert_eq!(&request.encode()[32..], &stream);
    }

    #[test]
    fn in_fences_are_serialized_before_command_stream() {
        let stream = vec![0xaa, 0xbb, 0xcc, 0xdd];
        let request = Submit3DCommand::with_in_fences(7, vec![11, 22], stream.clone());
        let encoded = request.encode();

        assert_eq!(request.request.num_in_fences, 2);
        assert_eq!(&encoded[32..40], &11u64.to_le_bytes());
        assert_eq!(&encoded[40..48], &22u64.to_le_bytes());
        assert_eq!(&encoded[48..], &stream);
    }

    #[test]
    fn empty_command_stream_is_supported() {
        let request = Submit3DCommand::new(1, Vec::new());

        assert_eq!(request.request.size, 0);
        assert!(request.command_stream.is_empty());
        assert_eq!(request.encode().len(), Submit3D::SIZE);
    }
}
