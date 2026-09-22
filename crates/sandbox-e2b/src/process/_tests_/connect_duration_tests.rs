//! Duration-bound regressions for the public Connect transport.

use std::{sync::Arc, time::Duration};

use sandbox_interface::Error as DomainError;
use unimock::Unimock;

use crate::{
    ProcessCommand, ProcessConnection, ProcessOutputCapture, ProcessRegularFileRequest,
    ProcessTransport, SplitProcessCommand, StreamProcessCommand,
    process::http::ConnectHttpTransport,
};

use super::ConnectProcessTransport;

#[tokio::test]
async fn process_transport_rejects_excessive_durations_without_panicking() {
    let transport = process_transport();

    let connect_error = transport
        .connect(connection(), 7, Duration::MAX, 32)
        .await
        .expect_err("excessive connect wait must fail");
    let run_error = transport
        .run(
            connection(),
            ProcessCommand {
                command: "/bin/true".to_owned(),
                args: Vec::new(),
                cwd: None,
                output_capture: ProcessOutputCapture::HardLimit { max_bytes: 0 },
                timeout: Duration::MAX,
                read_only: false,
            },
        )
        .await
        .expect_err("excessive process timeout must fail");
    let split_error = transport
        .run_split(
            connection(),
            SplitProcessCommand {
                command: "/bin/true".to_owned(),
                args: Vec::new(),
                stdout_limit: 0,
                stderr_limit: 0,
                deadline: Duration::MAX,
            },
        )
        .await
        .expect_err("excessive split-process deadline must fail");
    let file_error = transport
        .read_regular_file(
            connection(),
            ProcessRegularFileRequest {
                root: "/tmp/sandbox".to_owned(),
                path: "file".to_owned(),
                offset: 0,
                max_bytes: 32,
                timeout: Duration::MAX,
            },
        )
        .await
        .expect_err("excessive regular-file timeout must fail");
    let stream_result = transport
        .stream_process(
            connection(),
            StreamProcessCommand {
                requested_at: tokio::time::Instant::now(),
                command: "/bin/true".to_owned(),
                args: Vec::new(),
                stdout_limit: 0,
                stderr_limit: 0,
                deadline: Duration::MAX,
                idle_timeout: Duration::from_secs(1),
            },
        )
        .await;
    let stream_error = match stream_result {
        Err(error) => error,
        Ok(_) => panic!("excessive stream deadline must fail"),
    };

    for error in [connect_error, run_error, split_error, file_error] {
        assert!(matches!(
            error,
            DomainError::InvalidSeconds {
                minimum: 0,
                maximum: 300,
                ..
            }
        ));
    }
    assert!(matches!(
        stream_error,
        DomainError::InvalidSeconds {
            field: "deadline",
            minimum: 0,
            maximum: 3600,
        }
    ));
}

#[tokio::test]
async fn process_transport_rejects_invalid_stream_idle_timeout() {
    let result = process_transport()
        .stream_process(
            connection(),
            StreamProcessCommand {
                requested_at: tokio::time::Instant::now(),
                command: "/bin/true".to_owned(),
                args: Vec::new(),
                stdout_limit: 0,
                stderr_limit: 0,
                deadline: Duration::from_secs(30),
                idle_timeout: Duration::ZERO,
            },
        )
        .await;

    assert!(matches!(
        result,
        Err(DomainError::InvalidProcessIdleTimeout)
    ));
}

fn process_transport() -> ConnectProcessTransport {
    let http: Arc<dyn ConnectHttpTransport> = Arc::new(Unimock::new(()));
    ConnectProcessTransport {
        http,
        backend_id: "configured-e2b".to_owned(),
    }
}

fn connection() -> ProcessConnection {
    ProcessConnection::new(
        "sandbox".to_owned(),
        "e2b.app".to_owned(),
        "access-token".to_owned(),
    )
}
