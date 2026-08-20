#![cfg(feature = "virglrenderer-backend")]

#[cfg(test)]
mod tests {
    use crate::virtio_gpu::protocol::commands::*;
    use crate::virtio_gpu::protocol::header::CtrlHeader;
    use crate::virtio_gpu::protocol::requests::capset::{GetCapset, GetCapsetInfo};

    use super::super::runtime::VenusRuntime;

    #[test]
    fn capset_requests_preserve_request_metadata() {
        let header = CtrlHeader {
            typ: CMD_GET_CAPSET_INFO,
            flags: FLAG_FENCE,
            fence_id: 42,
            ctx_id: 7,
            ring_idx: 3,
            padding: [0; 3],
        };
        let mut bytes = GetCapsetInfo::new(0).encode_le().to_vec();
        bytes[..24].copy_from_slice(&header.encode_le());
        assert_eq!(CtrlHeader::decode_le(&bytes).unwrap(), header);

        let request = GetCapset::venus(1);
        let decoded = GetCapset::decode_le(&request.encode_le()).unwrap();
        assert_eq!(decoded.capset_id, CAPSET_VENUS);
    }

    #[test]
    fn runtime_type_is_constructible_without_execution_assumption() {
        fn assert_runtime_type(_: fn() -> Result<VenusRuntime, super::super::runtime::VenusRuntimeError>) {}
        assert_runtime_type(VenusRuntime::new);
    }
}
