//! Diagnostics exclude process data while trusted callers retain exact bytes.

use std::{collections::BTreeMap, time::Duration};

use sandbox_interface::{
    BackendRunProcessRequest, BackendStreamProcessRequest, Error, ImageCommandFailure,
    ProcessStreamEvent, ProcessStreamOutcome, ProviderRef, ResourceOwner, RetainedSandboxRef,
    RunProcessRequest, SandboxId, SandboxProcessOutput, StreamProcessRequest,
};
use uuid::Uuid;

#[test]
fn request_diagnostics_contain_only_approved_metadata() {
    for payload in [
        "credential_alpha",
        "prefix-password123",
        "token/a+b",
        "\x1b[31msecret",
    ] {
        let backend = request(payload);
        let service = RunProcessRequest {
            owner: ResourceOwner::platform(Uuid::nil()),
            sandbox_id: SandboxId::new(),
            command: backend.command.clone(),
            args: backend.args.clone(),
            cwd: backend.cwd.clone(),
            envs: backend.envs.clone(),
            stdout_limit: backend.stdout_limit,
            stderr_limit: backend.stderr_limit,
            deadline: backend.deadline,
        };

        assert_eq!(
            format!("{backend:?}"),
            "BackendRunProcessRequest { arg_count: 1, has_cwd: true, env_count: 1, \
             stdout_limit: 128, stderr_limit: 256, deadline: 1s }"
        );
        for debug in [
            format!("{backend:#?}"),
            format!("{service:?}"),
            format!("{service:#?}"),
        ] {
            assert!(!debug.contains(payload));
            assert!(!debug.contains("command:"));
            assert!(!debug.contains("envs:"));
        }
        assert_eq!(backend.envs.get(payload).map(String::as_str), Some(payload));
        assert_eq!(service.args, [payload]);
    }
}

#[test]
fn stream_request_debug_omits_command_and_arguments() {
    let secret = "credential_alpha";
    let backend = BackendStreamProcessRequest {
        sandbox_provider_ref: ProviderRef::new(secret),
        command: secret.to_owned(),
        args: vec![secret.to_owned()],
        cwd: Some(format!("/{secret}")),
        envs: BTreeMap::from([(secret.to_owned(), secret.to_owned())]),
        stdout_limit: 128,
        stderr_limit: 256,
        deadline: Duration::from_secs(5),
        idle_timeout: Duration::from_secs(1),
    };
    let service = StreamProcessRequest {
        owner: ResourceOwner::platform(Uuid::nil()),
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
    assert_eq!(
        format!("{backend:?}"),
        "BackendStreamProcessRequest { arg_count: 1, has_cwd: true, env_count: 1, \
         stdout_limit: 128, stderr_limit: 256, deadline: 5s, idle_timeout: 1s }"
    );
    for diagnostic in [
        format!("{backend:?}"),
        format!("{backend:#?}"),
        format!("{service:?}"),
        format!("{service:#?}"),
    ] {
        assert!(!diagnostic.contains(secret), "{diagnostic}");
        assert!(!diagnostic.contains("command:"), "{diagnostic}");
        assert!(!diagnostic.contains("args:"), "{diagnostic}");
        assert!(!diagnostic.contains(" cwd:"), "{diagnostic}");
        assert!(!diagnostic.contains("envs:"), "{diagnostic}");
    }
    let diagnostic = format!("{service:?}");
    assert!(diagnostic.contains("owner:"), "{diagnostic}");
    assert!(diagnostic.contains("sandbox_id:"), "{diagnostic}");
    assert!(diagnostic.contains("idle_timeout: 1s"), "{diagnostic}");
    assert_eq!(service.args, [secret]);
    assert_eq!(service.command, secret);
    assert_eq!(service.cwd, backend.cwd);
    assert_eq!(service.envs, backend.envs);
    assert_eq!(backend.cwd.as_deref(), Some("/credential_alpha"));
    assert_eq!(backend.envs.get(secret).map(String::as_str), Some(secret));
}

#[test]
fn stream_event_debug_omits_output_bytes() {
    let secret = b"credential_alpha\xff".to_vec();
    for event in [
        ProcessStreamEvent::Stdout(secret.clone()),
        ProcessStreamEvent::Stderr(secret.clone()),
    ] {
        let diagnostic = format!("{event:?}");
        assert!(
            diagnostic.contains(&format!("output_bytes: {}", secret.len())),
            "{diagnostic}"
        );
        assert!(!diagnostic.contains("credential_alpha"), "{diagnostic}");
        assert!(!format!("{event:#?}").contains("credential_alpha"));
        assert!(
            matches!(event, ProcessStreamEvent::Stdout(bytes) | ProcessStreamEvent::Stderr(bytes) if bytes == secret)
        );
    }
    assert_eq!(
        format!("{:?}", ProcessStreamEvent::Started { pid: 7 }),
        "Started { pid: 7 }"
    );
    assert_eq!(
        format!(
            "{:?}",
            ProcessStreamEvent::Exited {
                exit_code: 3,
                exited: true
            }
        ),
        "Exited { exit_code: 3, exited: true }"
    );
    assert_eq!(
        format!(
            "{:?}",
            ProcessStreamEvent::Outcome(ProcessStreamOutcome::IdleTimeout)
        ),
        "Outcome(IdleTimeout)"
    );
}

#[test]
fn validation_diagnostics_do_not_echo_environment_names() {
    for name in [
        "credential_alpha",
        "LD_credential_alpha",
        "DYLD_credential_alpha",
    ] {
        let mut request = request("credential_alpha");
        request.cwd = None;
        request.envs = BTreeMap::from([
            ("TOKEN".to_owned(), name.to_owned()),
            (name.to_owned(), "never-log-stream-value\0".to_owned()),
        ]);
        let backend_stream = BackendStreamProcessRequest {
            sandbox_provider_ref: request.sandbox_provider_ref.clone(),
            command: request.command.clone(),
            args: request.args.clone(),
            cwd: request.cwd.clone(),
            envs: request.envs.clone(),
            stdout_limit: request.stdout_limit,
            stderr_limit: request.stderr_limit,
            deadline: request.deadline,
            idle_timeout: Duration::from_secs(1),
        };
        let service_stream = StreamProcessRequest {
            owner: ResourceOwner::platform(Uuid::nil()),
            sandbox_id: SandboxId::new(),
            command: backend_stream.command.clone(),
            args: backend_stream.args.clone(),
            cwd: backend_stream.cwd.clone(),
            envs: backend_stream.envs.clone(),
            stdout_limit: backend_stream.stdout_limit,
            stderr_limit: backend_stream.stderr_limit,
            deadline: backend_stream.deadline,
            idle_timeout: backend_stream.idle_timeout,
        };
        for result in [
            request.validate_execution_context(),
            backend_stream.validate_execution_context(),
            service_stream.validate_execution_context(),
        ] {
            let error = result.expect_err("invalid context");
            for diagnostic in [
                error.to_string(),
                format!("{error:?}"),
                format!("{error:#?}"),
            ] {
                assert!(!diagnostic.contains(name), "{diagnostic}");
                assert!(
                    !diagnostic.contains("never-log-stream-value"),
                    "{diagnostic}"
                );
            }
        }
    }
}

#[test]
fn process_output_debug_omits_bytes_without_changing_them() {
    let bytes = b"credential_alpha\r\n\x1b[31m\xff".to_vec();
    let output = SandboxProcessOutput {
        stdout: bytes.clone(),
        stderr: bytes.clone(),
        exit_code: Some(7),
        exited: true,
        stdout_overflowed: false,
        stderr_overflowed: true,
    };

    assert_eq!(
        format!("{output:?}"),
        format!(
            "SandboxProcessOutput {{ stdout_bytes: {}, stderr_bytes: {}, exit_code: Some(7), \
             exited: true, stdout_overflowed: false, stderr_overflowed: true }}",
            bytes.len(),
            bytes.len()
        )
    );
    assert!(!format!("{output:#?}").contains("stdout:"));
    assert!(!format!("{output:#?}").contains("stderr:"));
    assert_eq!(output.stdout, bytes);
    assert_eq!(output.stderr, bytes);
}

#[test]
fn image_failure_serialization_has_no_output_text() {
    let failure = ImageCommandFailure {
        exit_code: Some(7),
        exited: true,
        output_bytes: 32,
        output_truncated: false,
    };
    let serialized = serde_json::to_value(&failure).expect("serializable failure");

    assert!(serialized.get("output").is_none(), "{serialized}");
    let error = Error::ImageSetupFailed {
        command: Some(failure),
        retained_sandbox: Some(RetainedSandboxRef {
            sandbox_id: SandboxId::new(),
            provider_ref: ProviderRef::new("sensitive-provider-handle"),
        }),
    };
    for diagnostic in [
        error.to_string(),
        format!("{error:?}"),
        format!("{error:#?}"),
    ] {
        assert!(!diagnostic.contains("prefix-"));
        assert!(!diagnostic.contains("sensitive-provider-handle"));
    }
}

fn request(payload: &str) -> BackendRunProcessRequest {
    BackendRunProcessRequest {
        sandbox_provider_ref: ProviderRef::new(payload),
        command: payload.to_owned(),
        args: vec![payload.to_owned()],
        cwd: Some(format!("/{payload}")),
        envs: BTreeMap::from([(payload.to_owned(), payload.to_owned())]),
        stdout_limit: 128,
        stderr_limit: 256,
        deadline: Duration::from_secs(1),
    }
}
