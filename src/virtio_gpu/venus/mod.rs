pub mod command;
pub mod context;
pub mod dispatcher;
pub mod resource;

pub mod state {
    include!(concat!(env!("OUT_DIR"), "/venus_state.rs"));
}

#[cfg(feature = "virglrenderer-backend")]
#[path = "runtime.rs"]
mod runtime_impl;

#[cfg(feature = "virglrenderer-backend")]
pub mod runtime {
    pub use super::runtime_wrapper::VenusRuntime;
    pub use super::runtime_impl::VenusRuntimeError;
}

#[cfg(feature = "virglrenderer-backend")]
mod runtime_wrapper;

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
