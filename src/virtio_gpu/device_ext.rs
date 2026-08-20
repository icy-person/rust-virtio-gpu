use crate::virtio_gpu::protocol::commands::{RESP_OK_EDID, RESP_OK_NODATA};
use crate::virtio_gpu::protocol::requests::standard::{GetEdid, SetScanoutBlob};
use crate::virtio_gpu::protocol::responses::{RespEdid, RespOkNoData};

impl VirtioGpuDevice {
    pub(crate) fn handle_detach_backing(&mut self, resource_id: u32) -> Result<(), DeviceError> {
        let resource = self
            .resource_mut(resource_id)
            .ok_or(DeviceError::InvalidResource)?;
        resource.backing.clear();
        Ok(())
    }

    pub(crate) fn handle_get_edid(&self, request: &[u8]) -> Result<Vec<u8>, DeviceError> {
        let request = GetEdid::decode_le(request).ok_or(DeviceError::InvalidRequest)?;
        if request.scanout as usize >= MAX_SCANOUTS {
            return Err(DeviceError::InvalidResource);
        }
        let scanout = &self.scanouts[request.scanout as usize];
        if !scanout.enabled {
            return Err(DeviceError::InvalidResource);
        }
        let resource = self
            .resource
            .get(scanout.resource_id)
            .ok_or(DeviceError::InvalidResource)?;

        let mut edid = [0u8; 128];
        edid[..8].copy_from_slice(&[0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00]);
        // Generic manufacturer/product identity for the emulator display.
        edid[8..10].copy_from_slice(&[0x4f, 0x41]);
        edid[10..12].copy_from_slice(&[0x01, 0x00]);
        edid[16] = 1;
        edid[17] = 1;
        edid[18] = 1;
        edid[19] = 4;

        let width_cm = ((resource.width as f32 / 96.0).max(1.0)).round() as u8;
        let height_cm = ((resource.height as f32 / 96.0).max(1.0)).round() as u8;
        edid[21] = width_cm;
        edid[22] = height_cm;
        edid[23] = 0x78;
        edid[24] = 0x0a;
        edid[25] = 0xcf;
        edid[26] = 0x74;
        edid[27] = 0xa3;

        let hactive = resource.width.min(4095) as u16;
        let vactive = resource.height.min(4095) as u16;
        edid[56..58].copy_from_slice(&hactive.to_le_bytes());
        edid[58..60].copy_from_slice(&vactive.to_le_bytes());

        edid[126] = 0;
        edid[127] = 0u8.wrapping_sub(edid[..127].iter().fold(0u8, |a, b| a.wrapping_add(*b)));

        let mut bytes = Vec::with_capacity(RespEdid::SIZE);
        bytes.extend_from_slice(
            &crate::virtio_gpu::protocol::header::CtrlHeader {
                typ: RESP_OK_EDID,
                ..request.header
            }
            .encode_le(),
        );
        bytes.extend_from_slice(&(edid.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        let mut payload = [0u8; crate::virtio_gpu::protocol::responses::EDID_SIZE];
        payload[..edid.len()].copy_from_slice(&edid);
        bytes.extend_from_slice(&payload);
        Ok(bytes)
    }

    pub(crate) fn handle_set_scanout_blob(
        &mut self,
        request: &[u8],
    ) -> Result<(), DeviceError> {
        let request = SetScanoutBlob::decode_le(request).ok_or(DeviceError::InvalidRequest)?;
        if request.scanout_id as usize >= MAX_SCANOUTS || request.width == 0 || request.height == 0 {
            return Err(DeviceError::InvalidResource);
        }
        let resource = self
            .resource
            .get(request.resource_id)
            .ok_or(DeviceError::InvalidResource)?;
        if request.width > resource.width || request.height > resource.height {
            return Err(DeviceError::InvalidResource);
        }
        let stride = request.strides[0] as usize;
        let offset = request.offsets[0] as usize;
        let required = stride
            .checked_mul(request.height as usize)
            .and_then(|v| offset.checked_add(v))
            .ok_or(DeviceError::InvalidResource)?;
        if stride < request.width as usize * 4 || required > resource.data.len() {
            return Err(DeviceError::InvalidResource);
        }

        self.scanouts[request.scanout_id as usize] = Scanout {
            enabled: true,
            resource_id: request.resource_id,
            width: request.width,
            height: request.height,
        };
        if self.display.is_none() {
            self.display = Some(Display::new(request.width as usize, request.height as usize));
        }
        Ok(())
    }

    pub(crate) fn ok_no_data_response(request: crate::virtio_gpu::protocol::header::CtrlHeader) -> Vec<u8> {
        RespOkNoData {
            header: crate::virtio_gpu::protocol::header::CtrlHeader {
                typ: RESP_OK_NODATA,
                ..request
            },
        }
        .encode_le()
        .to_vec()
    }
}
