//! Diagnostics exclude process data while trusted callers retain exact bytes.

use std::{collections::BTreeMap, time::Duration};

use sandbox_interface::{
    BackendRunProcessRequest, Error, ImageCommandFailure, ProviderRef, ResourceOwner,
    RetainedSandboxRef, RunProcessRequest, SandboxId, SandboxProcessOutput,
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
            (name.to_owned(), "invalid\0value".to_owned()),
        ]);
        let error = request
            .validate_execution_context()
            .expect_err("invalid context");

        for diagnostic in [
            error.to_string(),
            format!("{error:?}"),
            format!("{error:#?}"),
        ] {
            assert!(!diagnostic.contains(name), "{diagnostic}");
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
