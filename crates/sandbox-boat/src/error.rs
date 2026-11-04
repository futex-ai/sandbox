//! Typed failures at the Boat provider boundary.

use internal_error::InternalError;
use sandbox_interface::SandboxStartRefusal;
use thiserror::Error;

/// Boat transport and control failures interpreted by the adapter.
#[derive(Debug, Error, internal_error::ErrorContract)]
pub enum Error {
    /// Configuration, a route, or an input is invalid.
    #[error("[sandbox_boat/error] invalid configuration or request")]
    InvalidRequest,
    /// The requested provider resource does not exist.
    #[error("[sandbox_boat/error] provider resource was not found")]
    NotFound,
    /// The provider rejected authorization.
    #[error("[sandbox_boat/error] provider authorization failed")]
    Unauthorized,
    /// A read or idempotent operation is temporarily unavailable.
    #[error("[sandbox_boat/error] provider is unavailable")]
    Unavailable,
    /// A mutation may have reached the provider and must not be replayed.
    #[error("[sandbox_boat/error] mutation delivery is ambiguous")]
    DeliveryAmbiguous,
    /// A successful response did not confirm the requested operation.
    #[error("[sandbox_boat/error] invalid provider acknowledgment")]
    InvalidResponse,
    /// Provider pagination repeated, conflicted, or exceeded its bound.
    #[error("[sandbox_boat/error] invalid provider pagination")]
    InvalidPagination,
    /// A provider response exceeded its control-plane byte limit.
    #[error("[sandbox_boat/error] provider response exceeded its byte limit")]
    ResponseTooLarge,
    /// The account offers an eligible machine absent from the published catalogue.
    #[error("[sandbox_boat/error] eligible machine type is unknown")]
    UnknownMachineType,
    /// A source disables provider snapshot capture.
    #[error("[sandbox_boat/error] source sandbox does not permit snapshot capture")]
    SnapshotCaptureDisabled,
    /// A recognized definitive start refusal proves that no sandbox started.
    #[error("[sandbox_boat/error] start refused: {refusal}")]
    StartRefused {
        /// Provider-neutral refusal and supplied retry timing.
        refusal: SandboxStartRefusal,
    },
    /// Unexpected implementation failure with its typed source.
    #[error("[sandbox_boat/error] internal failure")]
    Internal(#[from] InternalError),
}

/// Result at the Boat transport and control boundary.
pub type Result<T> = std::result::Result<T, Error>;
