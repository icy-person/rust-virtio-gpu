pub mod attach_backing;
pub mod blob;
pub mod capset;
pub mod context;
pub mod flush;
pub mod resource;
pub mod scanout;
pub mod standard;
pub mod submit;
pub mod transfer;

pub use attach_backing::{VirtioGpuMemEntry, VirtioGpuResourceAttachBacking};
pub use blob::{MemEntry, ResourceCreateBlob, ResourceMapBlob, ResourceUnmapBlob};
pub use capset::{CapsetInfo, CapsetResponse, GetCapset, GetCapsetInfo};
pub use context::{ContextAttachResource, ContextCreate, ContextDestroy, ContextDetachResource};
pub use standard::{
    Box3D, CursorPos, GetEdid, MoveCursor, ResourceAssignUuid, ResourceCreate3D,
    ResourceDetachBacking, ResourceUnref, SetScanoutBlob, TransferHost3D, UpdateCursor,
};
pub use submit::{Submit3D, Submit3DCommand};
pub use crate::virtio_gpu::protocol::flush::ResourceFlush;
pub use crate::virtio_gpu::protocol::responses::Rect;
pub use resource::ResourceCreate2D;
pub use scanout::ResourceSetScanout;
pub use transfer::ResourceTransferToHost2D;
