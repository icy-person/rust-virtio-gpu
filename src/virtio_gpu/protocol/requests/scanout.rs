pub struct ResourceSetScanout {
    pub scanout_id: u32,
    pub resource_id: u32,

    pub rect: [u32; 4],
}

impl ResourceSetScanout {
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() < 48 {
            return None;
        }

        let scanout_id = u32::from_le_bytes(data[24..28].try_into().ok()?);

        let resource_id = u32::from_le_bytes(data[28..32].try_into().ok()?);

        let x = u32::from_le_bytes(data[32..36].try_into().ok()?);

        let y = u32::from_le_bytes(data[36..40].try_into().ok()?);

        let width = u32::from_le_bytes(data[40..44].try_into().ok()?);

        let height = u32::from_le_bytes(data[44..48].try_into().ok()?);

        Some(Self {
            scanout_id,
            resource_id,
            rect: [x, y, width, height],
        })
    }
}
