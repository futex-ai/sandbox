use std::{collections::HashMap, sync::Arc};

use sandbox_interface::{BackendReadFileRequest, ProviderRef, SandboxBackend as _};
use unimock::{MockFn as _, Unimock, matching};

use crate::{
    ControlSandboxReadAccess, E2bAdapterConfig, E2bControlApiMock, E2bProfile, ProcessFileChunk,
    ProcessFileValidation, ProcessRegularFileRequest, ProcessTransportMock,
    backend::configured::E2bSandboxBackend,
};

use super::validate_target;

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

#[test]
fn symlink_escape_is_rejected_after_provider_canonicalization() {
    let validation = ProcessFileValidation {
        canonical_root: "/workspace/repo".to_owned(),
        canonical_path: "/etc/passwd".to_owned(),
        exists: true,
        regular: true,
        symlink: false,
        size: 100,
    };
    assert!(matches!(
        validate_target(&validation, false),
        Err(sandbox_interface::Error::FileOutsideRoot)
    ));
}

#[test]
fn in_root_symlink_is_rejected_without_following_its_regular_target() {
    let validation = ProcessFileValidation {
        canonical_root: "/workspace/repo".to_owned(),
        canonical_path: "/workspace/repo/target.txt".to_owned(),
        exists: true,
        regular: true,
        symlink: true,
        size: 100,
    };
    assert!(matches!(
        validate_target(&validation, false),
        Err(sandbox_interface::Error::FileNotRegular)
    ));
}

#[test]
fn regular_file_and_new_in_root_target_are_accepted() {
    let existing = ProcessFileValidation {
        canonical_root: "/workspace/repo".to_owned(),
        canonical_path: "/workspace/repo/src/lib.rs".to_owned(),
        exists: true,
        regular: true,
        symlink: false,
        size: 100,
    };
    assert!(validate_target(&existing, false).is_ok());
    let new_file = ProcessFileValidation {
        canonical_root: "/workspace/repo".to_owned(),
        canonical_path: "/workspace/repo/src/new.rs".to_owned(),
        exists: false,
        regular: false,
        symlink: false,
        size: 0,
    };
    assert!(validate_target(&new_file, true).is_ok());
}

#[test]
fn directory_is_rejected() {
    let validation = ProcessFileValidation {
        canonical_root: "/workspace/repo".to_owned(),
        canonical_path: "/workspace/repo/src".to_owned(),
        exists: true,
        regular: false,
        symlink: false,
        size: 0,
    };
    assert!(matches!(
        validate_target(&validation, true),
        Err(sandbox_interface::Error::FileNotRegular)
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
