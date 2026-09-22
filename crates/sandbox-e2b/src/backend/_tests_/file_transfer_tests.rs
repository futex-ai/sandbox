use std::{collections::HashMap, sync::Arc};

use sandbox_interface::{
    BackendReadFileRequest, BackendWriteFileRequest, Error, FILE_TRANSFER_MAX_BYTES,
    FILE_TRANSFER_PATH_MAX_BYTES, ProviderRef, SandboxBackend as _,
};
use unimock::{MockFn as _, Unimock, matching};

use crate::{
    ControlSandboxReadAccess, E2bAdapterConfig, E2bControlApiMock, E2bProfile, ProcessFileChunk,
    ProcessRegularFileRequest, ProcessRegularFileWriteRequest, ProcessTransportMock,
    backend::configured::E2bSandboxBackend,
};

#[tokio::test]
async fn read_uses_non_mutating_access_and_one_atomic_process_operation() {
    let control = Unimock::new(
        E2bControlApiMock::get_sandbox_read_access
            .next_call(matching!("sandbox"))
            .returns(Ok(ControlSandboxReadAccess {
                sandbox_id: "sandbox".to_owned(),
                domain: "e2b.app".to_owned(),
                envd_access_token: "token".to_owned(),
            })),
    );
    let processes = Unimock::new(
        ProcessTransportMock::read_regular_file
            .next_call(matching!(_, _))
            .answers(&|_, _, request: ProcessRegularFileRequest| {
                assert_eq!(request.root, "/workspace/repo");
                assert_eq!(request.path, "src/lib.rs");
                assert_eq!(request.offset, 2);
                assert_eq!(request.max_bytes, 4);
                Ok(ProcessFileChunk {
                    bytes: b"cdef".to_vec(),
                    total_size: 6,
                })
            }),
    );
    let backend = backend(control, processes);

    let content = backend
        .read_file(BackendReadFileRequest {
            sandbox_provider_ref: ProviderRef::new("sandbox"),
            root: "/workspace/repo".to_owned(),
            path: "src/lib.rs".to_owned(),
            offset: 2,
            max_bytes: 4,
        })
        .await
        .expect("atomic regular-file read");

    assert_eq!(content.bytes, b"cdef");
    assert_eq!(content.total_size, 6);
}

#[tokio::test]
async fn write_uses_one_atomic_descriptor_relative_process_operation() {
    let control = Unimock::new(
        E2bControlApiMock::connect_sandbox
            .next_call(matching!("sandbox"))
            .returns(Ok(crate::ControlSandboxAccess {
                sandbox_id: "sandbox".to_owned(),
                domain: "e2b.app".to_owned(),
                envd_access_token: "token".to_owned(),
                traffic_access_token: Some("traffic-token".to_owned()),
            })),
    );
    let processes = Unimock::new(
        ProcessTransportMock::write_regular_file
            .next_call(matching!(_, _))
            .answers(&|_, _, request: ProcessRegularFileWriteRequest| {
                assert_eq!(request.root, "/workspace/repo");
                assert_eq!(request.path, "src/lib.rs");
                assert_eq!(request.bytes, b"replacement");
                Ok(())
            }),
    );
    let backend = backend(control, processes);

    backend
        .write_file(BackendWriteFileRequest {
            sandbox_provider_ref: ProviderRef::new("sandbox"),
            root: "/workspace/repo".to_owned(),
            path: "src/lib.rs".to_owned(),
            bytes: b"replacement".to_vec(),
        })
        .await
        .expect("atomic regular-file replacement");
}

#[tokio::test]
async fn oversized_write_fails_before_connecting_to_the_provider() {
    let backend = backend(Unimock::new(()), Unimock::new(()));

    let error = backend
        .write_file(BackendWriteFileRequest {
            sandbox_provider_ref: ProviderRef::new("sandbox"),
            root: "/workspace/repo".to_owned(),
            path: "src/lib.rs".to_owned(),
            bytes: vec![0; FILE_TRANSFER_MAX_BYTES + 1],
        })
        .await
        .expect_err("oversized write should fail without provider work");

    assert!(matches!(
        error,
        Error::FileTooLarge {
            limit: FILE_TRANSFER_MAX_BYTES
        }
    ));
}

#[tokio::test]
async fn oversized_transfer_paths_fail_before_provider_access() {
    let backend = backend(Unimock::new(()), Unimock::new(()));
    let oversized = "x".repeat(FILE_TRANSFER_PATH_MAX_BYTES + 1);

    let read = backend
        .read_file(BackendReadFileRequest {
            sandbox_provider_ref: ProviderRef::new("sandbox"),
            root: format!("/{oversized}"),
            path: "file.txt".to_owned(),
            offset: 0,
            max_bytes: 1,
        })
        .await;
    let write = backend
        .write_file(BackendWriteFileRequest {
            sandbox_provider_ref: ProviderRef::new("sandbox"),
            root: "/workspace".to_owned(),
            path: oversized,
            bytes: Vec::new(),
        })
        .await;

    assert!(matches!(
        read,
        Err(Error::TextTooLarge {
            field: "root",
            limit: FILE_TRANSFER_PATH_MAX_BYTES
        })
    ));
    assert!(matches!(
        write,
        Err(Error::TextTooLarge {
            field: "path",
            limit: FILE_TRANSFER_PATH_MAX_BYTES
        })
    ));
}

#[tokio::test]
async fn malformed_transfer_paths_fail_before_provider_access() {
    let backend = backend(Unimock::new(()), Unimock::new(()));

    let read = backend
        .read_file(BackendReadFileRequest {
            sandbox_provider_ref: ProviderRef::new("sandbox"),
            root: "workspace".to_owned(),
            path: "file.txt".to_owned(),
            offset: 0,
            max_bytes: 1,
        })
        .await;
    let write = backend
        .write_file(BackendWriteFileRequest {
            sandbox_provider_ref: ProviderRef::new("sandbox"),
            root: "/workspace".to_owned(),
            path: "../outside.txt".to_owned(),
            bytes: Vec::new(),
        })
        .await;

    assert!(matches!(
        read,
        Err(Error::InvalidFilePath { field: "root" })
    ));
    assert!(matches!(
        write,
        Err(Error::InvalidFilePath { field: "path" })
    ));
}

fn backend(control: Unimock, processes: Unimock) -> E2bSandboxBackend {
    E2bSandboxBackend::with_transports(config(), Arc::new(control), Arc::new(processes))
}

fn config() -> E2bAdapterConfig {
    E2bAdapterConfig::new(
        "configured-e2b",
        "https://api.e2b.app",
        "api-key",
        HashMap::from([(
            "general".to_owned(),
            E2bProfile {
                template: "base".to_owned(),
                allow_public_egress: false,
                denied_destinations: vec!["203.0.113.10/32".to_owned()],
            },
        )]),
        600,
    )
    .expect("valid E2B config")
}
