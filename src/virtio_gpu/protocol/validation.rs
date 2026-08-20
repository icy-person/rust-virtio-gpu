use crate::virtio_gpu::features::GpuFeatures;
use crate::virtio_gpu::protocol::commands::{
    BLOB_FLAG_USE_CROSS_DEVICE, BLOB_FLAG_USE_MAPPABLE, BLOB_FLAG_USE_SHAREABLE, BLOB_MEM_GUEST,
    BLOB_MEM_HOST3D, BLOB_MEM_HOST3D_GUEST, FLAG_FENCE, FLAG_INFO_RING_IDX,
};
use crate::virtio_gpu::protocol::formats::VirtioGpuFormat;
use crate::virtio_gpu::protocol::responses::Rect;
use crate::virtio_gpu::protocol::{CONTEXT_INIT_CAPSET_ID_MASK, CtrlHeader};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValidationError {
    UnknownFormat,
    ZeroDimension,
    RectangleOutOfBounds,
    IntegerOverflow,
    InvalidFeatureCombination,
    InvalidBlobMemory,
    InvalidBlobFlags,
    BlobEntryCountMismatch,
    BlobBackingTooSmall,
    InvalidAlignment,
    InvalidRingIndex,
    RingFlagWithoutContextInit,
    ContextCapabilityMismatch,
    FenceIdRequired,
}

pub fn validate_format(value: u32) -> Result<VirtioGpuFormat, ValidationError> {
    VirtioGpuFormat::from_u32(value).ok_or(ValidationError::UnknownFormat)
}

pub fn validate_dimensions(width: u32, height: u32) -> Result<(), ValidationError> {
    if width == 0 || height == 0 {
        return Err(ValidationError::ZeroDimension);
    }

    (width as u64)
        .checked_mul(height as u64)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or(ValidationError::IntegerOverflow)?;

    Ok(())
}

pub fn validate_rect(
    resource_width: u32,
    resource_height: u32,
    rect: Rect,
) -> Result<(), ValidationError> {
    validate_dimensions(resource_width, resource_height)?;

    let right = (rect.x as u64)
        .checked_add(rect.width as u64)
        .ok_or(ValidationError::IntegerOverflow)?;
    let bottom = (rect.y as u64)
        .checked_add(rect.height as u64)
        .ok_or(ValidationError::IntegerOverflow)?;

    if right > resource_width as u64 || bottom > resource_height as u64 {
        return Err(ValidationError::RectangleOutOfBounds);
    }

    Ok(())
}

pub fn validate_transfer_2d(
    resource_width: u32,
    resource_height: u32,
    rect: Rect,
    offset: u64,
    backing_size: u64,
) -> Result<u64, ValidationError> {
    validate_rect(resource_width, resource_height, rect)?;

    let row_bytes = (rect.width as u64)
        .checked_mul(4)
        .ok_or(ValidationError::IntegerOverflow)?;
    let transfer_bytes = row_bytes
        .checked_mul(rect.height as u64)
        .ok_or(ValidationError::IntegerOverflow)?;
    let end = offset
        .checked_add(transfer_bytes)
        .ok_or(ValidationError::IntegerOverflow)?;

    if end > backing_size {
        return Err(ValidationError::BlobBackingTooSmall);
    }

    Ok(transfer_bytes)
}

pub fn validate_features(features: GpuFeatures) -> Result<(), ValidationError> {
    if !features.is_valid() {
        return Err(ValidationError::InvalidFeatureCombination);
    }

    Ok(())
}

pub fn validate_header(
    header: CtrlHeader,
    context_init_enabled: bool,
) -> Result<(), ValidationError> {
    if header.ring_idx >= 64 {
        return Err(ValidationError::InvalidRingIndex);
    }

    if header.flags & FLAG_INFO_RING_IDX != 0 && !context_init_enabled {
        return Err(ValidationError::RingFlagWithoutContextInit);
    }

    if header.flags & FLAG_FENCE != 0 && header.fence_id == 0 {
        return Err(ValidationError::FenceIdRequired);
    }

    Ok(())
}

pub fn validate_context_init(
    context_init: u32,
    supported_capsets: &[u32],
) -> Result<(), ValidationError> {
    let capset_id = context_init & CONTEXT_INIT_CAPSET_ID_MASK;

    if capset_id == 0 {
        return Ok(());
    }

    if supported_capsets.iter().any(|id| *id == capset_id) {
        Ok(())
    } else {
        Err(ValidationError::ContextCapabilityMismatch)
    }
}

pub fn validate_blob(
    blob_mem: u32,
    blob_flags: u32,
    nr_entries: u32,
    entries_len: usize,
    size: u64,
    alignment: Option<u64>,
    entry_bytes: u64,
) -> Result<(), ValidationError> {
    match blob_mem {
        BLOB_MEM_GUEST | BLOB_MEM_HOST3D | BLOB_MEM_HOST3D_GUEST => {}
        _ => return Err(ValidationError::InvalidBlobMemory),
    }

    let known_flags = BLOB_FLAG_USE_MAPPABLE | BLOB_FLAG_USE_SHAREABLE | BLOB_FLAG_USE_CROSS_DEVICE;
    if blob_flags & !known_flags != 0 {
        return Err(ValidationError::InvalidBlobFlags);
    }

    if nr_entries as usize != entries_len {
        return Err(ValidationError::BlobEntryCountMismatch);
    }

    if size == 0 {
        return Err(ValidationError::BlobBackingTooSmall);
    }

    if let Some(alignment) = alignment {
        if alignment == 0 || !alignment.is_power_of_two() {
            return Err(ValidationError::InvalidAlignment);
        }

        if size % alignment != 0 {
            return Err(ValidationError::InvalidAlignment);
        }
    }

    if matches!(blob_mem, BLOB_MEM_GUEST | BLOB_MEM_HOST3D_GUEST) && entry_bytes < size {
        return Err(ValidationError::BlobBackingTooSmall);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::virtio_gpu::protocol::{CAPSET_VENUS, CONTEXT_INIT_CAPSET_ID_MASK};

    #[test]
    fn rectangle_bounds_are_checked_without_overflow() {
        assert!(
            validate_rect(
                100,
                100,
                Rect {
                    x: 90,
                    y: 90,
                    width: 10,
                    height: 10,
                }
            )
            .is_ok()
        );

        assert_eq!(
            validate_rect(
                100,
                100,
                Rect {
                    x: 90,
                    y: 90,
                    width: 11,
                    height: 10,
                }
            ),
            Err(ValidationError::RectangleOutOfBounds)
        );
    }

    #[test]
    fn transfer_size_is_validated_against_backing() {
        let rect = Rect {
            x: 0,
            y: 0,
            width: 10,
            height: 2,
        };

        assert_eq!(validate_transfer_2d(10, 2, rect, 0, 80), Ok(80));
        assert_eq!(
            validate_transfer_2d(10, 2, rect, 1, 80),
            Err(ValidationError::BlobBackingTooSmall)
        );
    }

    #[test]
    fn context_capability_is_checked() {
        assert!(
            validate_context_init(CAPSET_VENUS & CONTEXT_INIT_CAPSET_ID_MASK, &[CAPSET_VENUS])
                .is_ok()
        );

        assert_eq!(
            validate_context_init(99, &[CAPSET_VENUS]),
            Err(ValidationError::ContextCapabilityMismatch)
        );
    }

    #[test]
    fn header_fence_rules_are_checked() {
        let good = CtrlHeader::new(1).with_fence(1);
        assert!(validate_header(good, true).is_ok());

        let bad = CtrlHeader::new(1).with_fence(0);
        assert_eq!(
            validate_header(bad, true),
            Err(ValidationError::FenceIdRequired)
        );
    }

    #[test]
    fn blob_validation_catches_entry_mismatch() {
        assert_eq!(
            validate_blob(BLOB_MEM_GUEST, 0, 2, 1, 4096, Some(4096), 4096),
            Err(ValidationError::BlobEntryCountMismatch)
        );
    }
}
