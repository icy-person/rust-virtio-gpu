use bitflags::bitflags;

bitflags! {
    /// VirtIO-GPU feature bits, including standard transport features that are
    /// negotiated through the same 64-bit feature bitmap.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct GpuFeatures: u64 {
        /// 3D / VirGL mode.
        const VIRGL = 1 << 0;

        /// EDID support.
        const EDID = 1 << 1;

        /// Resource UUID support.
        const RESOURCE_UUID = 1 << 2;

        /// Blob resources.
        const RESOURCE_BLOB = 1 << 3;

        /// Multiple context types and synchronization timelines.
        /// Requires VIRGL.
        const CONTEXT_INIT = 1 << 4;

        /// `blob_alignment` configuration field is valid.
        /// Requires RESOURCE_BLOB.
        const BLOB_ALIGNMENT = 1 << 5;

        /// VirtIO 1.x transport semantics.
        const VERSION_1 = 1 << 32;

        /// Queue reset is supported when negotiated.
        const RING_RESET = 1 << 40;
    }
}

impl GpuFeatures {
    pub fn is_valid(self) -> bool {
        (!self.contains(Self::CONTEXT_INIT) || self.contains(Self::VIRGL))
            && (!self.contains(Self::BLOB_ALIGNMENT) || self.contains(Self::RESOURCE_BLOB))
    }

    pub fn supported_subset(self, offered: Self) -> Self {
        self & offered
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feature_values_match_spec() {
        assert_eq!(GpuFeatures::VIRGL.bits(), 1 << 0);
        assert_eq!(GpuFeatures::EDID.bits(), 1 << 1);
        assert_eq!(GpuFeatures::RESOURCE_UUID.bits(), 1 << 2);
        assert_eq!(GpuFeatures::RESOURCE_BLOB.bits(), 1 << 3);
        assert_eq!(GpuFeatures::CONTEXT_INIT.bits(), 1 << 4);
        assert_eq!(GpuFeatures::BLOB_ALIGNMENT.bits(), 1 << 5);
        assert_eq!(GpuFeatures::VERSION_1.bits(), 1u64 << 32);
        assert_eq!(GpuFeatures::RING_RESET.bits(), 1u64 << 40);
    }

    #[test]
    fn context_init_requires_virgl() {
        assert!(!GpuFeatures::CONTEXT_INIT.is_valid());
        assert!((GpuFeatures::VIRGL | GpuFeatures::CONTEXT_INIT).is_valid());
    }

    #[test]
    fn blob_alignment_requires_blob_resources() {
        assert!(!GpuFeatures::BLOB_ALIGNMENT.is_valid());
        assert!((GpuFeatures::RESOURCE_BLOB | GpuFeatures::BLOB_ALIGNMENT).is_valid());
    }

    #[test]
    fn feature_negotiation_is_intersection() {
        let offered = GpuFeatures::VIRGL | GpuFeatures::RESOURCE_BLOB | GpuFeatures::VERSION_1;
        let requested = GpuFeatures::VIRGL
            | GpuFeatures::EDID
            | GpuFeatures::RESOURCE_BLOB
            | GpuFeatures::VERSION_1;
        assert_eq!(
            requested.supported_subset(offered),
            GpuFeatures::VIRGL | GpuFeatures::RESOURCE_BLOB | GpuFeatures::VERSION_1
        );
    }

    #[test]
    fn ring_reset_is_independently_negotiable() {
        let offered = GpuFeatures::VERSION_1 | GpuFeatures::RING_RESET;
        let requested = GpuFeatures::VERSION_1;
        assert!(!requested.contains(GpuFeatures::RING_RESET));
        assert!(offered.contains(GpuFeatures::RING_RESET));
    }
}
