//! Caller text stays outside diagnostic formatting across execution surfaces.

use std::{fmt::Debug, time::Duration};

use sandbox_interface::{
    ActionId, BackendInputRequest, BackendOutputRequest, BackendPrepareImageRequest,
    BackendReadOnlyExecRequest, BackendTerminal, BackendTerminalCreateRequest,
    CreateTerminalRequest, ExecuteTerminalRequest, ImageSource, OperationId, ProviderRef,
    ReadOnlyExecOutput, ReadOnlyExecRequest, RealizeImageFileInput, RealizeImageRequest,
    ResourceOwner, SandboxId, TerminalId, TerminalState, WriteTerminalRequest,
};
use uuid::Uuid;

const PAYLOAD: &str = "sensitive-caller-data";

#[test]
fn read_only_execution_omits_text_and_keeps_output_exact() {
    let request = ReadOnlyExecRequest {
        owner: owner(),
        sandbox_id: SandboxId::new(),
        cwd: PAYLOAD.to_owned(),
        executable: PAYLOAD.to_owned(),
        args: vec![PAYLOAD.to_owned()],
        output_limit: 1024,
        timeout_seconds: 1,
    };
    let backend = BackendReadOnlyExecRequest {
        sandbox_provider_ref: provider(),
        cwd: PAYLOAD.to_owned(),
        executable: PAYLOAD.to_owned(),
        args: vec![PAYLOAD.to_owned()],
        output_limit: 1024,
        timeout: Duration::from_secs(1),
    };
    let output = ReadOnlyExecOutput {
        bytes: PAYLOAD.as_bytes().to_vec(),
        exit_code: 7,
    };

    for value in [&request as &dyn Debug, &backend, &output] {
        assert_metadata_only(value);
    }
    assert_eq!(
        format!("{backend:?}"),
        "BackendReadOnlyExecRequest { arg_count: 1, output_limit: 1024, timeout: 1s }"
    );
    assert_eq!(output.bytes, PAYLOAD.as_bytes());
}

#[test]
fn terminal_requests_and_provider_metadata_omit_caller_text() {
    let create = CreateTerminalRequest {
        owner: owner(),
        operation_id: OperationId::new(),
        lifecycle_operation_id: None,
        sandbox_id: SandboxId::new(),
        cwd: Some(PAYLOAD.to_owned()),
    };
    let backend_create = BackendTerminalCreateRequest {
        terminal_id: TerminalId::new(),
        operation_id: OperationId::new(),
        sandbox_provider_ref: provider(),
        cwd: Some(PAYLOAD.to_owned()),
        provider_log_limit: 1024,
    };
    let read = BackendOutputRequest {
        sandbox_provider_ref: provider(),
        terminal_provider_ref: provider(),
        provider_log_path: PAYLOAD.to_owned(),
        offset: 0,
        max_bytes: 1024,
        provider_log_limit: 1024,
        wait: Duration::ZERO,
    };
    let terminal = BackendTerminal {
        provider_ref: provider(),
        provider_log_path: PAYLOAD.to_owned(),
        state: TerminalState::Ready,
    };
    let execute = ExecuteTerminalRequest {
        owner: owner(),
        operation_id: OperationId::new(),
        terminal_id: TerminalId::new(),
        command: PAYLOAD.to_owned(),
        timeout_seconds: Some(1),
    };
    let write = WriteTerminalRequest {
        owner: owner(),
        sandbox_id: SandboxId::new(),
        operation_id: OperationId::new(),
        action_id: ActionId::new(),
        input: PAYLOAD.to_owned(),
        wait_timeout_seconds: None,
    };
    let backend_write = BackendInputRequest {
        sandbox_provider_ref: provider(),
        terminal_provider_ref: provider(),
        input: PAYLOAD.as_bytes().to_vec(),
    };
    for value in [
        &create as &dyn Debug,
        &backend_create,
        &read,
        &terminal,
        &execute,
        &write,
        &backend_write,
    ] {
        assert_metadata_only(value);
    }
    assert_eq!(write.input, PAYLOAD);
    assert_eq!(backend_write.input, PAYLOAD.as_bytes());
}

#[test]
fn image_request_debug_omits_scripts_paths_and_file_contents() {
    let file = RealizeImageFileInput {
        root: PAYLOAD.to_owned(),
        path: PAYLOAD.to_owned(),
        bytes: PAYLOAD.as_bytes().to_vec(),
    };
    let backend = BackendPrepareImageRequest {
        sandbox_id: SandboxId::new(),
        source_provider_ref: provider(),
        owner: owner(),
        input_files: vec![file.clone()],
        setup_script: PAYLOAD.to_owned(),
        verify_commands: vec![PAYLOAD.to_owned()],
    };
    let service = RealizeImageRequest {
        owner: owner(),
        operation_id: OperationId::new(),
        source: ImageSource::Profile(PAYLOAD.to_owned()),
        input_files: vec![file.clone()],
        setup_script: PAYLOAD.to_owned(),
        verify_commands: vec![PAYLOAD.to_owned()],
    };
    for value in [&file as &dyn Debug, &backend, &service] {
        assert_metadata_only(value);
    }
    assert_eq!(backend.setup_script, PAYLOAD);
    assert_eq!(service.input_files[0].bytes, PAYLOAD.as_bytes());
}

#[test]
fn provider_ref_debug_hides_identity_without_changing_explicit_access() {
    let provider = provider();
    assert_metadata_only(&provider);
    assert_eq!(provider.as_str(), PAYLOAD);
    assert_eq!(
        serde_json::to_value(&provider).expect("provider serialization"),
        PAYLOAD
    );
}

fn assert_metadata_only(value: &dyn Debug) {
    for debug in [format!("{value:?}"), format!("{value:#?}")] {
        assert!(!debug.contains(PAYLOAD), "{debug}");
        for field in [
            "command:",
            "args:",
            "cwd:",
            "executable:",
            "input:",
            "bytes:",
            "setup_script:",
        ] {
            assert!(!debug.contains(&format!(" {field}")), "{debug}");
        }
    }
}

fn owner() -> ResourceOwner {
    ResourceOwner::platform(Uuid::nil())
}

fn provider() -> ProviderRef {
    ProviderRef::new(PAYLOAD)
}
