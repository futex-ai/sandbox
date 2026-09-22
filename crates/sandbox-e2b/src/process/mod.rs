//! E2B envd Process/PTY Connect transport.

mod connect;
mod connect_helpers;
mod connection;
mod duration;
mod file;
mod framing;
mod http;
mod http_error;
mod http_stream;
mod mapping;
mod provider_pid;
mod regular_file;
mod regular_file_cleanup;
mod regular_file_write;
mod selector;
mod split_run;
mod status;
mod stream_run;
mod stream_state;
mod types;
mod wire;

pub use self::connect::ConnectProcessTransport;
pub use self::connection::ProcessConnection;
pub use self::regular_file_write::ProcessRegularFileWriteRequest;
pub use self::selector::ProcessSelector;
pub use self::types::{
    DynProcessTransport, ProcessCommand, ProcessConnectOutput, ProcessFileChunk,
    ProcessFileValidation, ProcessInfo, ProcessOutputCapture, ProcessPtyRequest,
    ProcessRegularFileRequest, ProcessRunOutput, ProcessSplitOutput, ProcessTransport,
    ProcessTransportMock, SplitProcessCommand, StreamProcessCommand,
};

#[cfg(test)]
#[path = "_tests_/end_stream_tests.rs"]
mod end_stream_tests;
