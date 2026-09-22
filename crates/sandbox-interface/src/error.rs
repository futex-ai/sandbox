//! Shared handled error contract.

use internal_error::InternalError;
use thiserror::Error;

use crate::{
    ActionId, ImageCommandFailure, RetainedSandboxRef, SandboxConsumer, SandboxNetworkPolicy,
    SandboxState, SnapshotState, TerminalActionState, TerminalId, TerminalState,
    error_kinds::{QuotaKind, ResourceKind},
};

/// Errors returned by sandbox backends and lifecycle services.
#[derive(Debug, Error, internal_error::ErrorContract)]
pub enum Error {
    /// Both profile and snapshot were provided for sandbox creation.
    #[error("[sandbox_interface/error] profile and snapshot_id are mutually exclusive")]
    ConflictingSandboxSource,
    /// A logical profile is not configured for the selected backend.
    #[error("[sandbox_interface/error] sandbox profile is not configured")]
    UnknownProfile,
    /// A stable backend ID is absent from the runtime registry.
    #[error("[sandbox_interface/error] sandbox backend `{backend_id}` is not configured")]
    UnknownBackend {
        /// Stable configured backend ID.
        backend_id: String,
    },
    /// A stable backend ID was registered more than once.
    #[error(
        "[sandbox_interface/error] sandbox backend `{backend_id}` is registered more than once"
    )]
    DuplicateBackend {
        /// Duplicated backend ID.
        backend_id: String,
    },
    /// The requested consumer handle does not exist for the caller.
    #[error("[sandbox_interface/error] {resource:?} handle was not found")]
    NotFound {
        /// Missing resource category.
        resource: ResourceKind,
    },
    /// A handle exists but belongs to another execution owner.
    #[error("[sandbox_interface/error] {resource:?} handle is not owned by the caller")]
    AccessDenied {
        /// Denied resource category.
        resource: ResourceKind,
    },
    /// A code-owned quota would be exceeded.
    #[error("[sandbox_interface/error] {quota:?} quota of {limit} was reached")]
    QuotaExceeded {
        /// Quota category.
        quota: QuotaKind,
        /// Applied limit.
        limit: u32,
    },
    /// The requested action is invalid for the sandbox state.
    #[error("[sandbox_interface/error] sandbox state {state:?} does not allow this operation")]
    SandboxStateConflict {
        /// Current sandbox state.
        state: SandboxState,
    },
    /// The requested action is invalid for the snapshot state.
    #[error("[sandbox_interface/error] snapshot state {state:?} does not allow this operation")]
    SnapshotStateConflict {
        /// Current snapshot state.
        state: SnapshotState,
    },
    /// The requested action is invalid for the terminal state.
    #[error("[sandbox_interface/error] terminal state {state:?} does not allow this operation")]
    TerminalStateConflict {
        /// Current terminal state.
        state: TerminalState,
    },
    /// A concurrent transcript reader advanced the durable provider cursor.
    #[error("[sandbox_interface/error] terminal transcript cursor changed concurrently")]
    TranscriptCursorConflict,
    /// The requested per-session network policy is not supported.
    #[error("[sandbox_interface/error] network policy {policy:?} is not supported")]
    UnsupportedNetworkPolicy {
        /// Rejected typed per-session network policy.
        policy: SandboxNetworkPolicy,
    },
    /// One allowlist destination has an invalid IP, CIDR, or DNS shape.
    #[error("[sandbox_interface/error] egress destination is invalid")]
    InvalidEgressDestination,
    /// An allowlist entry overlaps a deployment-owned deny range.
    #[error("[sandbox_interface/error] egress destination conflicts with deployment policy")]
    EgressDestinationDenied,
    /// Recovery found a sandbox created with a different network policy.
    #[error("[sandbox_interface/error] recovered sandbox network policy does not match")]
    SandboxNetworkPolicyMismatch,
    /// The operation requires the runtime consumer class.
    #[error(
        "[sandbox_interface/error] {resource:?} operations require a runtime-consumer sandbox, not {consumer:?}"
    )]
    RuntimeConsumerRequired {
        /// Requested resource category.
        resource: ResourceKind,
        /// Consumer class of the rejected sandbox.
        consumer: SandboxConsumer,
    },
    /// Snapshot creation could not prove one provider result.
    #[error("[sandbox_interface/error] snapshot identity requires operator reconciliation")]
    SnapshotReconciliationRequired {
        /// Source runtime retained so the caller can reconcile without replaying work.
        retained_sandbox: Option<RetainedSandboxRef>,
    },
    /// Exact terminal input may have been delivered and cannot be replayed.
    #[error("[sandbox_interface/error] terminal input delivery is unknown")]
    DeliveryUnknown,
    /// A command exceeds the code-owned byte limit.
    #[error("[sandbox_interface/error] command exceeds the {limit}-byte limit")]
    CommandTooLarge {
        /// Maximum command bytes.
        limit: usize,
    },
    /// A stateless command's combined output exceeded its caller-owned bound.
    #[error("[sandbox_interface/error] read-only command output exceeds its byte limit")]
    ReadOnlyOutputTooLarge,
    /// Exact input exceeds the code-owned byte limit.
    #[error("[sandbox_interface/error] input exceeds the {limit}-byte limit")]
    InputTooLarge {
        /// Maximum input bytes.
        limit: usize,
    },
    /// A required text field is empty.
    #[error("[sandbox_interface/error] `{field}` must not be empty")]
    EmptyText {
        /// Request field name.
        field: &'static str,
    },
    /// A bounded byte-length field is outside its allowed range.
    #[error("[sandbox_interface/error] `{field}` must be between {minimum} and {maximum}")]
    InvalidLength {
        /// Request field name.
        field: &'static str,
        /// Inclusive lower bound.
        minimum: usize,
        /// Inclusive upper bound.
        maximum: usize,
    },
    /// A bounded seconds field exceeds its allowed range.
    #[error("[sandbox_interface/error] `{field}` must be between {minimum} and {maximum}")]
    InvalidSeconds {
        /// Request field name.
        field: &'static str,
        /// Inclusive lower bound.
        minimum: u64,
        /// Inclusive upper bound.
        maximum: u64,
    },
    /// A bounded text field exceeds its allowed byte length.
    #[error("[sandbox_interface/error] `{field}` exceeds the {limit}-byte limit")]
    TextTooLarge {
        /// Request field name.
        field: &'static str,
        /// Maximum UTF-8 byte length.
        limit: usize,
    },
    /// A transfer root or relative file path is structurally invalid.
    #[error("[sandbox_interface/error] sandbox file `{field}` is invalid")]
    InvalidFilePath {
        /// Invalid field.
        field: &'static str,
    },
    /// A transfer path resolves beyond its trusted root.
    #[error("[sandbox_interface/error] sandbox file path escapes its transfer root")]
    FileOutsideRoot,
    /// A transfer target is not a regular file.
    #[error("[sandbox_interface/error] sandbox file target is not a regular file")]
    FileNotRegular,
    /// A transfer exceeds the shared byte cap.
    #[error("[sandbox_interface/error] sandbox file exceeds the {limit}-byte limit")]
    FileTooLarge {
        /// Maximum transfer bytes.
        limit: usize,
    },
    /// A replacement write failed without proving that its writer was revoked.
    #[error("[sandbox_interface/error] sandbox file replacement termination is unconfirmed")]
    FileWriteUnconfirmed,
    /// Port zero cannot address a sandbox HTTP service.
    #[error("[sandbox_interface/error] port must be between 1 and 65535")]
    InvalidPort,
    /// A screen viewport pair is outside the shared exact-size domain.
    #[error(
        "[sandbox_interface/error] screen viewport {width}x{height} is outside the supported domain"
    )]
    InvalidScreenViewport {
        /// Rejected width in CSS pixels.
        width: u32,
        /// Rejected height in CSS pixels.
        height: u32,
    },
    /// The selected sandbox profile does not support screen viewport resize.
    #[error("[sandbox_interface/error] screen viewport resize is not supported")]
    ScreenViewportResizeUnsupported,
    /// Resize may still be running; callers must keep the session fenced.
    #[error("[sandbox_interface/error] screen resize termination is unconfirmed")]
    ScreenViewportResizeUnconfirmed,
    /// Image realization is restricted to workspace-platform ownership.
    #[error("[sandbox_interface/error] image realization requires platform ownership")]
    PlatformOwnerRequired,
    /// Terminal operations require an execution-agent or workspace-platform owner.
    #[error("[sandbox_interface/error] terminal operations require a runtime owner")]
    AgentOwnerRequired,
    /// A platform image setup script exited unsuccessfully.
    #[error("[sandbox_interface/error] platform image setup failed")]
    ImageSetupFailed {
        /// Safe command failure facts, when the backend captured them.
        command: Option<ImageCommandFailure>,
        /// Failed runtime retained by the backend for debugging, when available.
        retained_sandbox: Option<RetainedSandboxRef>,
    },
    /// One platform image verification command exited unsuccessfully.
    #[error("[sandbox_interface/error] platform image verification {index} failed")]
    ImageVerificationFailed {
        /// Zero-based verification command index.
        index: usize,
        /// Safe command failure facts, when the backend captured them.
        command: Option<ImageCommandFailure>,
        /// Failed runtime retained by the backend for debugging, when available.
        retained_sandbox: Option<RetainedSandboxRef>,
    },
    /// Platform image quiesce and scrub exited unsuccessfully.
    #[error("[sandbox_interface/error] platform image quiesce and scrub failed")]
    ImageScrubFailed,
    /// The backend could not observe a positive retained image size.
    #[error("[sandbox_interface/error] platform image size is unavailable")]
    ImageSizeUnavailable,
    /// A snapshot or terminal action conflicts with the source lifecycle lease.
    #[error("[sandbox_interface/error] sandbox lifecycle operation is already in progress")]
    LifecycleLeaseBusy,
    /// The selected terminal already has one active execute action.
    #[error(
        "[sandbox_interface/error] terminal {terminal_id} has active action {action_id} in state {state:?}"
    )]
    TerminalActionBusy {
        /// Occupied terminal.
        terminal_id: TerminalId,
        /// Execute action occupying the terminal.
        action_id: ActionId,
        /// Current execute action state.
        state: TerminalActionState,
    },
    /// A known terminal action no longer accepts the requested transition.
    #[error(
        "[sandbox_interface/error] terminal action {action_id} in state {state:?} does not allow this operation"
    )]
    TerminalActionStateConflict {
        /// Action that rejected the transition.
        action_id: ActionId,
        /// Current action state.
        state: TerminalActionState,
    },
    /// The provider reports that its control plane is temporarily unavailable.
    #[error("[sandbox_interface/error] backend `{backend_id}` is unavailable")]
    BackendUnavailable {
        /// Stable configured backend ID.
        backend_id: String,
    },
    /// Unexpected implementation or provider failure.
    #[error("[sandbox_interface/error] internal error")]
    Internal(#[from] InternalError),
}

/// Shared sandbox result alias.
pub type Result<T> = std::result::Result<T, Error>;
