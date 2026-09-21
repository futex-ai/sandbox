//! E2B implementation of the consumer's provider-neutral sandbox backend.
#![warn(missing_docs)]
#![warn(unreachable_pub)]

mod backend;
mod config;
mod control;
mod error;
mod process;

pub use self::backend::configured::E2bSandboxBackend;
pub use self::config::{E2bAdapterConfig, E2bProfile, E2bRuntimeConventions};
pub use self::control::{
    ControlCreateSandbox, ControlSandbox, ControlSandboxAccess, ControlSandboxReadAccess,
    ControlSandboxState, ControlSnapshot, DynE2bControlApi, E2bControlApi, E2bControlApiMock,
    ReqwestE2bControlApi, SandboxMetadata,
};
pub use self::error::{Error as E2bAdapterError, Result as E2bAdapterResult};
pub use self::process::{
    ConnectProcessTransport, DynProcessTransport, ProcessCommand, ProcessConnectOutput,
    ProcessConnection, ProcessFileChunk, ProcessFileValidation, ProcessInfo, ProcessOutputCapture,
    ProcessPtyRequest, ProcessRegularFileRequest, ProcessRunOutput, ProcessSplitOutput,
    ProcessTransport, ProcessTransportMock, SplitProcessCommand,
};
