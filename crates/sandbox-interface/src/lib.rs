//! Provider-neutral contracts for agent sandboxes, snapshots, and terminals.
#![warn(missing_docs)]
#![warn(unreachable_pub)]

mod backend;
mod backend_files;
mod backend_images;
mod backend_terminal;
mod capability_account_capacity;
mod capability_files;
mod capability_image_preparation;
mod capability_lifecycle;
mod capability_port_ingress;
mod capability_processes;
mod capability_resumable_lifecycle;
mod capability_screens;
mod capability_snapshots;
mod capability_terminals;
mod capacity;
pub mod conformance;
mod conformance_files;
mod conformance_image;
mod conformance_network;
mod conformance_probes;
mod conformance_process;
mod conformance_process_stream;
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
mod process_stream;
mod provider_ref;
mod read_only;
mod registry;
mod requests;
mod screen_stack;
mod service;
mod start_refusal;
mod states;

pub use self::backend::{
    BackendCreateSandboxRequest, BackendCreateSnapshotRequest, BackendInspectSnapshotRequest,
    BackendManagedSandbox, BackendSandbox, BackendSnapshot, BackendSnapshotCreateOutcome,
    BackendSnapshotInventory, BackendSnapshotRecovery,
};
pub use self::backend_files::{
    BackendFileContent, BackendReadFileRequest, BackendWriteFileRequest,
};
pub use self::backend_images::{BackendPrepareImageRequest, BackendPreparedImage};
pub use self::backend_terminal::{
    BackendInputRequest, BackendOutputRequest, BackendTerminal, BackendTerminalCreateRequest,
    BackendTerminalOutput, TERMINAL_OUTPUT_MAX_WAIT,
};
pub use self::capability_account_capacity::{SandboxAccountCapacity, SandboxAccountCapacityMock};
pub use self::capability_files::{SandboxFiles, SandboxFilesMock};
pub use self::capability_image_preparation::{
    SandboxImagePreparation, SandboxImagePreparationMock,
};
pub use self::capability_lifecycle::{SandboxLifecycle, SandboxLifecycleMock};
pub use self::capability_port_ingress::{SandboxPortIngress, SandboxPortIngressMock};
pub use self::capability_processes::{SandboxProcesses, SandboxProcessesMock};
pub use self::capability_resumable_lifecycle::{
    SandboxResumableLifecycle, SandboxResumableLifecycleMock,
};
pub use self::capability_screens::{SandboxScreens, SandboxScreensMock};
pub use self::capability_snapshots::{SandboxSnapshots, SandboxSnapshotsMock};
pub use self::capability_terminals::{SandboxTerminals, SandboxTerminalsMock};
pub use self::capacity::{
    ExactDecimal, ResourceAmount, ResourceUnit, SandboxPrice, SandboxPriceUnit, SandboxResources,
    SandboxSizeOffer, SandboxSizeSelection,
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
pub use self::lifetime::SandboxLifetime;
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
pub use self::process_stream::{
    BackendStreamProcessRequest, PROCESS_STREAM_MAX_DEADLINE, ProcessEventStream,
    ProcessStreamEvent, ProcessStreamOutcome, StreamProcessRequest,
};
pub use self::provider_ref::ProviderRef;
pub use self::read_only::{
    BackendReadOnlyExecRequest, DynSandboxReadOnly, InspectReadOnlySandboxRequest,
    ReadOnlyExecOutput, ReadOnlyExecRequest, ReadOnlyFileOutput, ReadOnlyFileRequest,
    ReadOnlySandbox, SandboxReadOnly, SandboxReadOnlyMock,
};
pub use self::registry::{SandboxRegistry, SandboxRegistryMock};
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
pub use self::start_refusal::{
    BillingRefusalReason, PlanSizeRefusalReason, SandboxStartRefusal, SandboxStartRetry,
    TemporaryStartRefusalReason,
};
pub use self::states::{
    CleanupState, SandboxState, SnapshotState, TerminalActionKind, TerminalActionState,
    TerminalState,
};
