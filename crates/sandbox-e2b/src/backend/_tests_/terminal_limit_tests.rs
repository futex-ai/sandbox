//! Pre-dispatch terminal transcript-limit regressions.

use std::{collections::HashMap, sync::Arc};

use sandbox_interface::{
    BackendTerminalCreateRequest, Error, FILE_TRANSFER_MAX_BYTES, OperationId, ProviderRef,
    SandboxBackend, TerminalId,
};
use unimock::Unimock;

use crate::{E2bAdapterConfig, E2bProfile};

use super::configured::E2bSandboxBackend;

#[tokio::test]
async fn oversized_terminal_transcript_limit_fails_before_provider_access() {
    let backend = E2bSandboxBackend::with_transports(
        config(),
        Arc::new(Unimock::new(())),
        Arc::new(Unimock::new(())),
    );
    let request = BackendTerminalCreateRequest {
        terminal_id: TerminalId::new(),
        operation_id: OperationId::new(),
        sandbox_provider_ref: ProviderRef::new("sandbox"),
        cwd: None,
        provider_log_limit: FILE_TRANSFER_MAX_BYTES + 1,
    };

    let create_error = backend
        .create_terminal(request.clone())
        .await
        .expect_err("oversized terminal creation must fail before provider access");
    let recover_error = backend
        .recover_terminal_create(request)
        .await
        .expect_err("oversized terminal recovery must fail before provider access");

    assert_limit_error(create_error);
    assert_limit_error(recover_error);
}

fn assert_limit_error(error: Error) {
    assert!(matches!(
        error,
        Error::InvalidLength {
            field: "provider_log_limit",
            minimum: 0,
            maximum: FILE_TRANSFER_MAX_BYTES,
        }
    ));
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
    .expect("valid adapter config")
}
