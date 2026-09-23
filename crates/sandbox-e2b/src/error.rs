//! Adapter-local typed transport failures.

use internal_error::InternalError;
use thiserror::Error;

/// E2B adapter failures interpreted at the provider boundary.
#[derive(Debug, Error, internal_error::ErrorContract)]
pub enum Error {
    /// The requested E2B resource does not exist.
    #[error("[sandbox_e2b/error] provider resource was not found")]
    NotFound,
    /// The provider rejected authentication or authorization.
    #[error("[sandbox_e2b/error] provider authentication failed")]
    Unauthorized,
    /// An E2B request is invalid or was rejected by the provider.
    #[error("[sandbox_e2b/error] E2B request is invalid or was rejected")]
    InvalidRequest,
    /// The provider is temporarily unavailable or rate limited.
    #[error("[sandbox_e2b/error] provider is temporarily unavailable")]
    Unavailable,
    /// A mutating request may have reached the provider.
    #[error("[sandbox_e2b/error] provider delivery outcome is ambiguous")]
    DeliveryAmbiguous,
    /// A Connect response frame was malformed.
    #[error("[sandbox_e2b/error] malformed Connect response frame")]
    MalformedFrame,
    /// A bounded provider response exceeded its adapter limit.
    #[error("[sandbox_e2b/error] provider response exceeded its limit")]
    ResponseTooLarge,
    /// The provider returned an unsafe or non-terminating pagination sequence.
    #[error("[sandbox_e2b/error] provider returned invalid pagination")]
    InvalidPagination,
    /// Unexpected adapter or transport failure.
    #[error("[sandbox_e2b/error] internal error")]
    Internal(#[from] InternalError),
}

/// Result returned by provider-specific E2B control and wire operations.
pub type Result<T> = std::result::Result<T, Error>;
