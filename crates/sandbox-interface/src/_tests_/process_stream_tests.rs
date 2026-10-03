//! Streaming context validation matches collected runs at both request layers.

use std::{collections::BTreeMap, time::Duration};

use uuid::Uuid;

use crate::{
    BackendRunProcessRequest, BackendStreamProcessRequest, Error, FILE_TRANSFER_PATH_MAX_BYTES,
    PROCESS_RUN_MAX_ENV_BYTES, PROCESS_RUN_MAX_ENV_VARS, ProcessRunContextError, ProviderRef,
    ResourceOwner, Result, SandboxId, StreamProcessRequest,
};

#[test]
fn absent_context_is_valid_for_both_stream_layers() {
    assert_context(None, BTreeMap::new(), None);
}

#[test]
fn exact_byte_bounds_and_safe_names_are_accepted() {
    assert_context(
        Some(format!("/{}", "w".repeat(FILE_TRANSFER_PATH_MAX_BYTES - 1))),
        BTreeMap::from([("A".to_owned(), "x".repeat(PROCESS_RUN_MAX_ENV_BYTES - 1))]),
        None,
    );
    assert_context(
        Some("/workspace/Å".to_owned()),
        BTreeMap::from([
            ("_".to_owned(), "line one\nline two".to_owned()),
            ("a9_SAFE".to_owned(), "\u{7f}".to_owned()),
        ]),
        None,
    );
}

#[test]
fn invalid_working_directories_have_the_collected_run_reasons() {
    for cwd in [
        "",
        "workspace",
        "/workspace/\0secret",
        "/workspace\n/child",
        "/\u{7f}",
    ] {
        assert_context(
            Some(cwd.to_owned()),
            BTreeMap::new(),
            Some(ProcessRunContextError::InvalidWorkingDirectory),
        );
    }
    assert_context(
        Some(format!("/{}", "w".repeat(FILE_TRANSFER_PATH_MAX_BYTES))),
        BTreeMap::new(),
        Some(ProcessRunContextError::WorkingDirectoryTooLarge {
            limit: FILE_TRANSFER_PATH_MAX_BYTES,
        }),
    );
}

#[test]
fn invalid_and_template_owned_names_have_the_collected_run_reasons() {
    for name in ["", "1FIRST", "HAS-DASH", "NON_ASCII_Å"] {
        assert_environment_error(
            name,
            "value",
            ProcessRunContextError::InvalidEnvironmentName,
        );
    }
    for name in [
        "PATH",
        "HOME",
        "LD_PRELOAD",
        "LD_LIBRARY_PATH",
        "LD_AUDIT",
        "DYLD_INSERT_LIBRARIES",
    ] {
        assert_environment_error(
            name,
            "secret",
            ProcessRunContextError::TemplateOwnedEnvironmentName,
        );
    }
}

#[test]
fn nul_values_have_the_collected_run_reason() {
    assert_environment_error(
        "SANDBOX_PROBE",
        "before\0secret-after",
        ProcessRunContextError::InvalidEnvironmentValue,
    );
}

#[test]
fn environment_count_accepts_the_bound_and_rejects_one_more() {
    let mut envs: BTreeMap<String, String> = (0..PROCESS_RUN_MAX_ENV_VARS)
        .map(|index| (format!("SAFE_{index}"), String::new()))
        .collect();
    assert_context(None, envs.clone(), None);
    envs.insert(format!("SAFE_{PROCESS_RUN_MAX_ENV_VARS}"), String::new());
    assert_context(
        None,
        envs,
        Some(ProcessRunContextError::TooManyEnvironmentVariables {
            limit: PROCESS_RUN_MAX_ENV_VARS,
        }),
    );
}

#[test]
fn environment_bytes_reject_one_byte_over_the_bound() {
    assert_environment_error(
        "A",
        &"x".repeat(PROCESS_RUN_MAX_ENV_BYTES),
        ProcessRunContextError::EnvironmentTooLarge {
            limit: PROCESS_RUN_MAX_ENV_BYTES,
        },
    );
}

fn assert_environment_error(name: &str, value: &str, expected: ProcessRunContextError) {
    assert_context(
        None,
        BTreeMap::from([(name.to_owned(), value.to_owned())]),
        Some(expected),
    );
}

fn assert_context(
    cwd: Option<String>,
    envs: BTreeMap<String, String>,
    expected: Option<ProcessRunContextError>,
) {
    let collected = BackendRunProcessRequest {
        sandbox_provider_ref: ProviderRef::new("provider"),
        command: "/bin/true".to_owned(),
        args: Vec::new(),
        cwd,
        envs,
        stdout_limit: 4096,
        stderr_limit: 4096,
        deadline: Duration::from_secs(30),
    };
    let backend = BackendStreamProcessRequest {
        sandbox_provider_ref: collected.sandbox_provider_ref.clone(),
        command: collected.command.clone(),
        args: collected.args.clone(),
        cwd: collected.cwd.clone(),
        envs: collected.envs.clone(),
        stdout_limit: collected.stdout_limit,
        stderr_limit: collected.stderr_limit,
        deadline: collected.deadline,
        idle_timeout: Duration::from_secs(1),
    };
    let service = StreamProcessRequest {
        owner: ResourceOwner::agent(Uuid::nil(), Uuid::nil()),
        sandbox_id: SandboxId::new(),
        command: backend.command.clone(),
        args: backend.args.clone(),
        cwd: backend.cwd.clone(),
        envs: backend.envs.clone(),
        stdout_limit: backend.stdout_limit,
        stderr_limit: backend.stderr_limit,
        deadline: backend.deadline,
        idle_timeout: backend.idle_timeout,
    };
    let collected_reason = context_reason(collected.validate_execution_context());
    assert_eq!(collected_reason, expected);
    assert_eq!(
        context_reason(backend.validate_execution_context()),
        collected_reason
    );
    assert_eq!(
        context_reason(service.validate_execution_context()),
        collected_reason
    );
}

fn context_reason(result: Result<()>) -> Option<ProcessRunContextError> {
    match result {
        Ok(()) => None,
        Err(Error::InvalidProcessRunContext { reason }) => Some(reason),
        Err(error) => panic!("unexpected validation error: {error:?}"),
    }
}
