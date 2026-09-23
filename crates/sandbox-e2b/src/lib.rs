//! E2B implementation of the consumer's provider-neutral sandbox backend.
#![warn(missing_docs)]
#![warn(unreachable_pub)]

mod backend;
mod config;
mod control;
mod error;
mod network;
mod process;
mod response_body;
mod runtime_conventions;
mod trusted_python;

pub use self::backend::configured::E2bSandboxBackend;
pub use self::config::{E2bAdapterConfig, E2bProfile};
pub use self::control::{
    ControlCreateSandbox, ControlSandbox, ControlSandboxAccess, ControlSandboxReadAccess,
    ControlSandboxState, ControlSnapshot, DynE2bControlApi, E2bControlApi, E2bControlApiMock,
    ReqwestE2bControlApi, SandboxMetadata,
};
pub use self::error::{Error as E2bAdapterError, Result as E2bAdapterResult};
pub use self::process::{
    ConnectProcessTransport, DynProcessTransport, ProcessCommand, ProcessConnectOutput,
    ProcessConnection, ProcessFileChunk, ProcessFileValidation, ProcessInfo, ProcessOutputCapture,
    ProcessPtyRequest, ProcessRegularFileRequest, ProcessRegularFileWriteRequest, ProcessRunOutput,
    ProcessSelector, ProcessSplitOutput, ProcessTransport, ProcessTransportMock,
    SplitProcessCommand,
};
pub use self::runtime_conventions::E2bRuntimeConventions;
