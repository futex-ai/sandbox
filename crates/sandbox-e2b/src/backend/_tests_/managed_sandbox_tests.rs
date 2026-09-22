//! Deployment-managed sandbox metadata reconciliation tests.

use std::{collections::HashMap, sync::Arc};

use sandbox_interface::{OperationId, SandboxBackend, SandboxConsumer, SandboxId, SandboxState};
use unimock::{MockFn, Unimock, matching};

use crate::{
    ControlSandbox, ControlSandboxState, E2bAdapterConfig, E2bControlApiMock, E2bProfile,
    E2bRuntimeConventions,
};

use super::configured::E2bSandboxBackend;

#[tokio::test]
async fn managed_sandboxes_parse_only_valid_sandbox_correlation_metadata() {
    let sandbox_id = SandboxId::new();
    let operation_id = OperationId::new();
    let metadata = std::collections::BTreeMap::from([
        ("tenant_sandbox_id".to_owned(), sandbox_id.to_string()),
        ("tenant_operation_id".to_owned(), operation_id.to_string()),
        ("tenant_consumer".to_owned(), "browser".to_owned()),
    ]);
    let control = Unimock::new(
        E2bControlApiMock::list_sandboxes
            .next_call(matching!(_))
            .answers_arc(Arc::new(move |_, query| {
                assert_eq!(
                    query.get("tenant_deployment_id").map(String::as_str),
                    Some("deployment")
                );
                Ok(vec![
                    ControlSandbox {
                        sandbox_id: "provider-sandbox".to_owned(),
                        state: ControlSandboxState::Paused,
                        metadata: metadata.clone(),
                    },
                    ControlSandbox {
                        sandbox_id: "legacy-sandbox".to_owned(),
                        state: ControlSandboxState::Running,
                        metadata: Default::default(),
                    },
                ])
            })),
    );
    let backend =
        E2bSandboxBackend::with_transports(config(), Arc::new(control), Arc::new(Unimock::new(())));

    let managed = backend
        .list_managed_sandboxes("deployment".to_owned())
        .await
        .expect("managed sandbox list should map metadata");

    assert_eq!(managed.len(), 2);
    assert_eq!(managed[0].state, SandboxState::Paused);
    assert_eq!(managed[0].sandbox_id, Some(sandbox_id));
    assert_eq!(managed[0].operation_id, Some(operation_id));
    assert_eq!(managed[0].consumer, Some(SandboxConsumer::Browser));
    assert_eq!(managed[1].consumer, None);
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
    .with_runtime_conventions(
        E2bRuntimeConventions::new("tenant", "tenant-terminal-", "/opt/tenant/screen-helper")
            .unwrap(),
    )
}
