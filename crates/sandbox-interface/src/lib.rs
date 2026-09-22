//! Provider-neutral contracts for agent sandboxes, snapshots, and terminals.
#![warn(missing_docs)]
#![warn(unreachable_pub)]

mod backend;
mod backend_files;
mod backend_images;
mod backend_terminal;
pub mod conformance;
mod conformance_image;
mod conformance_network;
mod conformance_resources;
mod constants;
mod consumer;
mod diagnostics;
mod domain;
mod error;
mod error_kinds;
mod ids;
mod image_command_failure;
mod lifetime;
mod network;
mod port_ingress;
mod process_run;
mod provider_ref;
mod read_only;
mod registry;
mod requests;
mod screen_stack;
mod service;
mod states;

pub use self::backend::{
    BackendCreateSandboxRequest, BackendCreateSnapshotRequest, BackendInspectSnapshotRequest,
    BackendManagedSandbox, BackendSandbox, BackendSnapshot, BackendSnapshotCreateOutcome,
    BackendSnapshotInventory, BackendSnapshotRecovery, DynSandboxBackend, SandboxBackend,
    SandboxBackendMock,
};
pub use self::backend_files::{
    BackendFileContent, BackendReadFileRequest, BackendWriteFileRequest,
};
pub use self::backend_images::{BackendPrepareImageRequest, BackendPreparedImage};
pub use self::backend_terminal::{
    BackendInputRequest, BackendOutputRequest, BackendTerminal, BackendTerminalCreateRequest,
    BackendTerminalOutput, TERMINAL_OUTPUT_MAX_WAIT,
};
pub use self::constants::{
    FILE_TRANSFER_MAX_BYTES, FILE_TRANSFER_PATH_MAX_BYTES, IMAGE_COMMAND_OUTPUT_MAX_BYTES,
    SANDBOX_PROFILE_MAX_BYTES, SANDBOX_PROFILE_MAX_ITEMS, valid_sandbox_profile_name,
};
pub use self::consumer::SandboxConsumer;
pub use self::domain::{
    FileContent, RealizedImage, ResourceOwner, RetainedSandboxRef, Sandbox, SandboxSnapshot,
    Terminal, TerminalAction, TerminalActionResult, TerminalStatus, TranscriptWindow,
};
pub use self::error::{Error, Result};
pub use self::error_kinds::{QuotaKind, ResourceKind};
pub use self::ids::{ActionId, OperationId, SandboxId, SnapshotId, TerminalId};
pub use self::image_command_failure::ImageCommandFailure;
pub use self::lifetime::{SANDBOX_ONE_SHOT_MAX_LIFETIME, SandboxLifetime};
pub use self::network::{
    EGRESS_DESTINATION_MAX_ITEMS, EGRESS_DOMAIN_MAX_BYTES, EgressDestination, SandboxNetworkPolicy,
};
pub use self::port_ingress::{
    BackendPortIngressRequest, PortIngress, PortIngressCredential, PortIngressRequest,
};
pub use self::process_run::{
    BackendRunProcessRequest, PROCESS_RUN_MAX_ARGV_BYTES, PROCESS_RUN_MAX_DEADLINE,
    PROCESS_RUN_MAX_ENV_BYTES, PROCESS_RUN_MAX_ENV_VARS, PROCESS_RUN_MAX_STREAM_BYTES,
    ProcessRunContextError, RunProcessRequest, SandboxProcessOutput,
};
pub use self::provider_ref::ProviderRef;
pub use self::read_only::{
    BackendReadOnlyExecRequest, DynSandboxReadOnly, InspectReadOnlySandboxRequest,
    ReadOnlyExecOutput, ReadOnlyExecRequest, ReadOnlyFileOutput, ReadOnlyFileRequest,
    ReadOnlySandbox, SandboxReadOnly, SandboxReadOnlyMock,
};
pub use self::registry::{
    DynSandboxBackendRegistry, SandboxBackendRegistry, SandboxBackendRegistryMock,
};
pub use self::requests::{
    AdoptSandboxRequest, CloseTerminalRequest, CreateSandboxRequest, CreateSnapshotRequest,
    CreateTerminalRequest, DeleteSnapshotRequest, DestroySandboxRequest, ExecuteTerminalRequest,
    ImageSource, ListSandboxesRequest, ListSnapshotsRequest, ListTerminalsRequest, ReadFileRequest,
    ReadTerminalActionRequest, ReadTerminalRequest, RealizeImageFileInput, RealizeImageRequest,
    SandboxLifecycleRequest, SandboxTerminalReplacementRequest, TerminalStatusRequest,
    WriteFileRequest, WriteTerminalRequest,
};
pub use self::screen_stack::{
    BackendEnsureScreenStackRequest, BackendResizeScreenStackRequest, EnsureScreenStackRequest,
    ResizeScreenStackRequest, SCREEN_VIEWPORT_MAX_HEIGHT, SCREEN_VIEWPORT_MAX_PIXELS,
    SCREEN_VIEWPORT_MAX_WIDTH, SCREEN_VIEWPORT_MIN_HEIGHT, SCREEN_VIEWPORT_MIN_WIDTH,
    ScreenStackCapabilities, ScreenStackOutcome, ScreenViewportSize,
};
pub use self::service::{DynSandboxService, SandboxService, SandboxServiceMock};
pub use self::states::{
    CleanupState, SandboxState, SnapshotState, TerminalActionKind, TerminalActionState,
    TerminalState,
};
