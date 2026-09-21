//! E2B envd Process/PTY Connect transport.

mod connect;
mod connect_helpers;
mod file;
mod framing;
mod http;
mod mapping;
mod regular_file;
mod split_run;
mod types;
mod wire;

pub use self::connect::ConnectProcessTransport;
pub use self::types::{
    DynProcessTransport, ProcessCommand, ProcessConnectOutput, ProcessConnection, ProcessFileChunk,
    ProcessFileValidation, ProcessInfo, ProcessOutputCapture, ProcessPtyRequest,
    ProcessRegularFileRequest, ProcessRunOutput, ProcessSplitOutput, ProcessTransport,
    ProcessTransportMock, SplitProcessCommand,
};
