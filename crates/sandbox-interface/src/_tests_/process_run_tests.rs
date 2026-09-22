//! Direct process execution-context validation and diagnostic tests.

use std::{collections::BTreeMap, time::Duration};

use uuid::Uuid;

use crate::{
    BackendRunProcessRequest, Error, FILE_TRANSFER_PATH_MAX_BYTES, PROCESS_RUN_MAX_ENV_BYTES,
    PROCESS_RUN_MAX_ENV_VARS, ProcessRunContextError, ProviderRef, ResourceOwner,
    RunProcessRequest, SandboxId,
};

#[test]
fn valid_context_accepts_exact_bounds_and_safe_names() {
    let mut request = backend_request();
    request.cwd = Some(format!("/{}", "w".repeat(FILE_TRANSFER_PATH_MAX_BYTES - 1)));
    request.envs = BTreeMap::from([(
        "A".to_owned(),
        "x".repeat(PROCESS_RUN_MAX_ENV_BYTES - "A".len()),
    )]);

    request
        .validate_execution_context()
        .expect("exact byte bounds should be accepted");

    request.cwd = Some("/workspace/Å".to_owned());
    request.envs = BTreeMap::from([
        ("_".to_owned(), "line one\nline two".to_owned()),
        ("a9_SAFE".to_owned(), "\u{7f}".to_owned()),
    ]);
    request
        .validate_execution_context()
        .expect("safe names and non-NUL values should be accepted");
}

#[test]
fn absent_context_is_valid_for_both_request_layers() {
    backend_request()
        .validate_execution_context()
        .expect("backend defaults should remain valid");
    service_request()
        .validate_execution_context()
        .expect("service defaults should remain valid");
}

#[test]
fn cwd_must_be_absolute() {
    assert_context_error(
        request_with_cwd("workspace"),
        ProcessRunContextError::InvalidWorkingDirectory,
    );
}

#[test]
fn cwd_must_not_exceed_the_transfer_path_limit() {
    assert_context_error(
        request_with_cwd(&format!("/{}", "w".repeat(FILE_TRANSFER_PATH_MAX_BYTES))),
        ProcessRunContextError::WorkingDirectoryTooLarge {
            limit: FILE_TRANSFER_PATH_MAX_BYTES,
        },
    );
}

#[test]
fn cwd_rejects_nul() {
    assert_context_error(
        request_with_cwd("/workspace/\0secret"),
        ProcessRunContextError::InvalidWorkingDirectory,
    );
}

#[test]
fn cwd_rejects_control_characters() {
    assert_context_error(
        request_with_cwd("/workspace\n/child"),
        ProcessRunContextError::InvalidWorkingDirectory,
    );
}

#[test]
fn environment_names_follow_the_portable_identifier_shape() {
    for invalid in ["", "1FIRST", "HAS-DASH", "NON_ASCII_Å"] {
        let mut request = backend_request();
        request.envs.insert(invalid.to_owned(), "value".to_owned());

        assert_context_error(request, ProcessRunContextError::InvalidEnvironmentName);
    }
}

#[test]
fn environment_values_reject_nul_without_echoing_the_value() {
    let mut request = backend_request();
    request.envs.insert(
        "SANDBOX_PROBE".to_owned(),
        "before\0secret-after".to_owned(),
    );

    let error = request
        .validate_execution_context()
        .expect_err("NUL environment value should fail");

    assert!(matches!(
        error,
        Error::InvalidProcessRunContext {
            reason: ProcessRunContextError::InvalidEnvironmentValue
        }
    ));
    assert!(!error.to_string().contains("secret-after"));
}

#[test]
fn template_owned_environment_names_are_rejected() {
    for name in [
        "PATH",
        "HOME",
        "LD_PRELOAD",
        "LD_LIBRARY_PATH",
        "LD_AUDIT",
        "DYLD_INSERT_LIBRARIES",
    ] {
        let mut request = backend_request();
        request.envs.insert(name.to_owned(), "secret".to_owned());

        assert_context_error(
            request,
            ProcessRunContextError::TemplateOwnedEnvironmentName,
        );
    }
}

#[test]
fn environment_count_is_capped() {
    let mut request = backend_request();
    for index in 0..PROCESS_RUN_MAX_ENV_VARS {
        request.envs.insert(format!("SAFE_{index}"), String::new());
    }
    request
        .validate_execution_context()
        .expect("the exact environment count limit should be accepted");
    request
        .envs
        .insert(format!("SAFE_{PROCESS_RUN_MAX_ENV_VARS}"), String::new());

    assert_context_error(
        request,
        ProcessRunContextError::TooManyEnvironmentVariables {
            limit: PROCESS_RUN_MAX_ENV_VARS,
        },
    );
}

#[test]
fn total_environment_key_and_value_bytes_are_capped() {
    let mut request = backend_request();
    request.envs.insert(
        "A".to_owned(),
        "x".repeat(PROCESS_RUN_MAX_ENV_BYTES - "A".len() + 1),
    );

    assert_context_error(
        request,
        ProcessRunContextError::EnvironmentTooLarge {
            limit: PROCESS_RUN_MAX_ENV_BYTES,
        },
    );
}

#[test]
fn request_debug_reports_environment_count_only() {
    let envs = BTreeMap::from([("SANDBOX_TOKEN".to_owned(), "never-log-me".to_owned())]);
    let mut backend = backend_request();
    backend.envs = envs.clone();
    let mut service = service_request();
    service.envs = envs;

    for debug in [format!("{backend:?}"), format!("{service:?}")] {
        assert!(!debug.contains("SANDBOX_TOKEN"));
        assert!(debug.contains("env_count: 1"));
        assert!(!debug.contains("never-log-me"));
    }
}

fn request_with_cwd(cwd: &str) -> BackendRunProcessRequest {
    let mut request = backend_request();
    request.cwd = Some(cwd.to_owned());
    request
}

fn assert_context_error(request: BackendRunProcessRequest, expected: ProcessRunContextError) {
    assert!(matches!(
        request.validate_execution_context(),
        Err(Error::InvalidProcessRunContext { reason }) if reason == expected
    ));
}

fn backend_request() -> BackendRunProcessRequest {
    BackendRunProcessRequest {
        sandbox_provider_ref: ProviderRef::new("provider"),
        command: "/bin/true".to_owned(),
        args: Vec::new(),
        cwd: None,
        envs: BTreeMap::new(),
        stdout_limit: 4096,
        stderr_limit: 4096,
        deadline: Duration::from_secs(30),
    }
}

fn service_request() -> RunProcessRequest {
    RunProcessRequest {
        owner: ResourceOwner::agent(Uuid::nil(), Uuid::nil()),
        sandbox_id: SandboxId::new(),
        command: "/bin/true".to_owned(),
        args: Vec::new(),
        cwd: None,
        envs: BTreeMap::new(),
        stdout_limit: 4096,
        stderr_limit: 4096,
        deadline: Duration::from_secs(30),
    }
}
