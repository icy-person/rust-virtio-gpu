pub mod command;
pub mod context;
pub mod dispatcher;
pub mod resource;
pub mod state;

#[cfg(feature = "virglrenderer-backend")]
pub mod runtime;

#[cfg(feature = "virglrenderer-backend")]
pub mod virgl;

pub use dispatcher::{VenusDispatchError, VenusResponse};
pub use state::{
    BlobMemory, FencePoint, FenceTracker, VENUS_MAX_VERSION, VenusContext, VenusResource,
    VenusState, VenusStateError,
};

#[cfg(feature = "virglrenderer-backend")]
pub use runtime::{VenusRuntime, VenusRuntimeError};

#[cfg(feature = "virglrenderer-backend")]
pub use virgl::{CompletedFence, VirglVenusBackend};
