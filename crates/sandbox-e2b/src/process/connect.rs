//! Connect JSON implementation of the high-level process transport.

use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use sandbox_interface::{Error as DomainError, Result as DomainResult};
use serde::{Serialize, de::DeserializeOwned};

use crate::error::{Error, Result};

use super::{
    http::{ConnectHttpTransport, ReqwestConnectHttpTransport},
    mapping::{map_file_result, map_result},
    types::{
        ProcessCommand, ProcessConnectOutput, ProcessConnection, ProcessFileChunk,
        ProcessFileValidation, ProcessInfo, ProcessOutputCapture, ProcessPtyRequest,
        ProcessRegularFileRequest, ProcessRunOutput, ProcessSplitOutput, ProcessTransport,
        SplitProcessCommand,
    },
    wire::{EmptyWire, ListResponseWire, decode, encode, pty_start, selector, send_input, signal},
};

const START_TIMEOUT: Duration = Duration::from_secs(60);

/// Connect JSON envd process transport.
pub struct ConnectProcessTransport {
    pub(super) http: Arc<dyn ConnectHttpTransport>,
    pub(super) backend_id: String,
}

impl ConnectProcessTransport {
    /// Builds the production Reqwest-backed Connect transport.
    pub fn new(backend_id: impl Into<String>) -> DomainResult<Self> {
        let backend_id = backend_id.into();
        let http = match ReqwestConnectHttpTransport::new() {
            Ok(http) => http,
            Err(error) => return Err(super::mapping::map_error(error, false, &backend_id)),
        };
        Ok(Self {
            http: Arc::new(http),
            backend_id,
        })
    }

    async fn unary<T: Serialize, R: DeserializeOwned>(
        &self,
        connection: ProcessConnection,
        method: &str,
        request: &T,
        ambiguous: bool,
    ) -> Result<R> {
        let bytes = self
            .http
            .unary(connection, method.to_owned(), encode(request)?, ambiguous)
            .await?;
        decode(&bytes)
    }

    async fn unary_empty<T: Serialize>(
        &self,
        connection: ProcessConnection,
        method: &str,
        request: &T,
        ambiguous: bool,
    ) -> Result<()> {
        self.http
            .unary(connection, method.to_owned(), encode(request)?, ambiguous)
            .await?;
        Ok(())
    }
}

#[async_trait]
impl ProcessTransport for ConnectProcessTransport {
    async fn start_pty(
        &self,
        connection: ProcessConnection,
        request: ProcessPtyRequest,
    ) -> DomainResult<ProcessInfo> {
        let tag = request.tag.clone();
        let body = pty_start(request);
        let events = map_result(
            self.collect(
                connection,
                "Start",
                &body,
                START_TIMEOUT,
                ProcessOutputCapture::HardLimit { max_bytes: 0 },
                true,
            )
            .await,
            false,
            &self.backend_id,
        )?;
        let pid = events.pid.ok_or_else(|| {
            DomainError::internal_message("E2B PTY start ended before a start event")
        })?;
        Ok(ProcessInfo {
            pid,
            tag: Some(tag),
        })
    }

    async fn connect(
        &self,
        connection: ProcessConnection,
        pid: u32,
        wait: Duration,
        max_bytes: usize,
    ) -> DomainResult<ProcessConnectOutput> {
        let events = map_result(
            self.collect(
                connection,
                "Connect",
                &selector(pid),
                wait,
                ProcessOutputCapture::HardLimit { max_bytes },
                false,
            )
            .await,
            false,
            &self.backend_id,
        )?;
        Ok(ProcessConnectOutput {
            bytes: events.bytes,
            exit_code: events.exit_code,
            exited: events.exited,
        })
    }

    async fn run(
        &self,
        connection: ProcessConnection,
        command: ProcessCommand,
    ) -> DomainResult<ProcessRunOutput> {
        let timeout = command.timeout;
        self.run_for(connection, command, timeout).await
    }

    async fn run_split(
        &self,
        connection: ProcessConnection,
        command: SplitProcessCommand,
    ) -> DomainResult<ProcessSplitOutput> {
        map_result(
            self.collect_split(connection, command).await,
            false,
            &self.backend_id,
        )
    }

    async fn list(&self, connection: ProcessConnection) -> DomainResult<Vec<ProcessInfo>> {
        let response: ListResponseWire = map_result(
            self.unary(connection, "List", &EmptyWire {}, false).await,
            false,
            &self.backend_id,
        )?;
        Ok(response
            .processes
            .into_iter()
            .map(|process| ProcessInfo {
                pid: process.pid,
                tag: process.tag,
            })
            .collect())
    }

    async fn send_input(
        &self,
        connection: ProcessConnection,
        pid: u32,
        input: Vec<u8>,
    ) -> DomainResult<()> {
        map_result(
            self.unary_empty(connection, "SendInput", &send_input(pid, input), true)
                .await,
            true,
            &self.backend_id,
        )
    }

    async fn kill(&self, connection: ProcessConnection, pid: u32) -> DomainResult<()> {
        match self
            .unary_empty(connection, "SendSignal", &signal(pid), false)
            .await
        {
            Ok(()) | Err(Error::NotFound) => Ok(()),
            Err(error) => Err(super::mapping::map_error(error, false, &self.backend_id)),
        }
    }

    async fn read_file(
        &self,
        connection: ProcessConnection,
        path: String,
        offset: u64,
        max_bytes: usize,
        timeout: Duration,
    ) -> DomainResult<ProcessFileChunk> {
        super::file::read(self, connection, path, offset, max_bytes, timeout).await
    }

    async fn read_regular_file(
        &self,
        connection: ProcessConnection,
        request: ProcessRegularFileRequest,
    ) -> DomainResult<ProcessFileChunk> {
        super::regular_file::read(self, connection, request).await
    }

    async fn validate_file(
        &self,
        connection: ProcessConnection,
        root: String,
        path: String,
        allow_missing: bool,
    ) -> DomainResult<ProcessFileValidation> {
        super::file::validate(self, connection, root, path, allow_missing).await
    }

    async fn download_file(
        &self,
        connection: ProcessConnection,
        path: String,
        offset: u64,
        max_bytes: usize,
    ) -> DomainResult<Vec<u8>> {
        map_file_result(
            self.http
                .download(connection, path, offset, max_bytes)
                .await,
            &self.backend_id,
        )
    }

    async fn upload_file(
        &self,
        connection: ProcessConnection,
        path: String,
        bytes: Vec<u8>,
    ) -> DomainResult<()> {
        map_file_result(
            self.http.upload(connection, path, bytes).await,
            &self.backend_id,
        )
    }
}

#[cfg(test)]
#[path = "_tests_/connect_transport_tests.rs"]
mod connect_transport_tests;

#[cfg(test)]
#[path = "_tests_/process_capture_tests.rs"]
mod process_capture_tests;
