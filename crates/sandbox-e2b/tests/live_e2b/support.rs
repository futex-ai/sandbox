//! Shared live-test configuration, polling, resource tracking, and cleanup.

use std::{
    collections::HashMap,
    error::Error,
    io::{self, Read, Write},
    net::TcpListener,
    time::Duration,
};

use sandbox_e2b::{E2bAdapterConfig, E2bProfile, E2bSandboxBackend};
use sandbox_interface::{
    BackendCreateSandboxRequest, BackendCreateSnapshotRequest, BackendOutputRequest,
    BackendSandbox, BackendSnapshotRecovery, BackendTerminal, BackendTerminalCreateRequest,
    Error as SandboxError, OperationId, ProviderRef, ResourceOwner, SandboxBackend,
    SandboxConsumer, SandboxId, TerminalId,
};

const LOG_LIMIT: usize = 2 * 1024 * 1024;
const LIVE_BACKEND_ID: &str = "e2b-live";
const RECOVERY_ATTEMPTS: usize = 60;
const RECOVERY_INTERVAL: Duration = Duration::from_secs(1);

pub(super) type LiveResult<T> = Result<T, Box<dyn Error>>;

pub(super) fn ingress_client() -> reqwest::Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .build()
}

#[derive(Default)]
pub(super) struct LiveResources {
    pub(super) sandboxes: Vec<ProviderRef>,
    pub(super) sandbox_requests: Vec<BackendCreateSandboxRequest>,
    pub(super) terminals: Vec<(ProviderRef, ProviderRef)>,
    pub(super) snapshot: Option<ProviderRef>,
    pub(super) snapshot_request: Option<BackendCreateSnapshotRequest>,
}

impl LiveResources {
    pub(super) fn track_terminal(&mut self, sandbox: &ProviderRef, terminal: &BackendTerminal) {
        self.terminals
            .push((sandbox.clone(), terminal.provider_ref.clone()));
    }

    pub(super) async fn cleanup(&self, backend: &E2bSandboxBackend) -> LiveResult<()> {
        let mut first_error = None;
        for (sandbox, terminal) in self.terminals.iter().rev() {
            if let Err(error) = backend
                .close_terminal(sandbox.clone(), terminal.clone())
                .await
                && !matches!(error, SandboxError::NotFound { .. })
            {
                first_error.get_or_insert(error);
            }
        }
        let snapshot = if let Some(snapshot) = &self.snapshot {
            Some(snapshot.clone())
        } else if let Some(request) = &self.snapshot_request {
            match recover_snapshot(backend, request.clone()).await {
                Ok(snapshot) => Some(snapshot),
                Err(error) => {
                    first_error.get_or_insert(error);
                    None
                }
            }
        } else {
            None
        };
        if let Some(snapshot) = snapshot
            && let Err(error) = backend.delete_snapshot(snapshot.clone()).await
            && !matches!(error, SandboxError::NotFound { .. })
        {
            first_error.get_or_insert(error);
        }
        let mut sandboxes = self.sandboxes.clone();
        for request in &self.sandbox_requests {
            match recover_sandbox(backend, request.clone()).await {
                Ok(sandbox) if !sandboxes.contains(&sandbox.provider_ref) => {
                    sandboxes.push(sandbox.provider_ref);
                }
                Ok(_) => {}
                Err(error) => {
                    first_error.get_or_insert(error);
                }
            }
        }
        for sandbox in sandboxes.iter().rev() {
            if let Err(error) = backend.destroy_sandbox(sandbox.clone()).await
                && !matches!(error, SandboxError::NotFound { .. })
            {
                first_error.get_or_insert(error);
            }
        }
        match first_error {
            Some(error) => Err(Box::new(error)),
            None => Ok(()),
        }
    }
}

pub(super) async fn create_tracked_sandbox(
    backend: &E2bSandboxBackend,
    request: BackendCreateSandboxRequest,
    resources: &mut LiveResources,
) -> sandbox_interface::Result<BackendSandbox> {
    resources.sandbox_requests.push(request.clone());
    let sandbox = match backend.create_sandbox(request.clone()).await {
        Ok(sandbox) => sandbox,
        Err(_) => recover_sandbox(backend, request.clone()).await?,
    };
    resources
        .sandbox_requests
        .retain(|pending| pending != &request);
    if !resources.sandboxes.contains(&sandbox.provider_ref) {
        resources.sandboxes.push(sandbox.provider_ref.clone());
    }
    Ok(sandbox)
}

async fn recover_sandbox(
    backend: &E2bSandboxBackend,
    request: BackendCreateSandboxRequest,
) -> sandbox_interface::Result<BackendSandbox> {
    for attempt in 0..=RECOVERY_ATTEMPTS {
        match backend.recover_sandbox_create(request.clone()).await? {
            Some(sandbox) => return Ok(sandbox),
            None if attempt == RECOVERY_ATTEMPTS => {
                return Err(SandboxError::BackendUnavailable {
                    backend_id: LIVE_BACKEND_ID.to_owned(),
                });
            }
            None => tokio::time::sleep(RECOVERY_INTERVAL).await,
        }
    }
    Err(SandboxError::BackendUnavailable {
        backend_id: LIVE_BACKEND_ID.to_owned(),
    })
}

pub(super) async fn recover_snapshot(
    backend: &E2bSandboxBackend,
    request: BackendCreateSnapshotRequest,
) -> sandbox_interface::Result<ProviderRef> {
    for attempt in 0..=RECOVERY_ATTEMPTS {
        match backend.recover_snapshot_create(request.clone()).await? {
            BackendSnapshotRecovery::Recovered(snapshot) => return Ok(snapshot.provider_ref),
            BackendSnapshotRecovery::ReconciliationRequired => {
                return Err(SandboxError::SnapshotReconciliationRequired {
                    retained_sandbox: None,
                });
            }
            BackendSnapshotRecovery::InProgress if attempt == RECOVERY_ATTEMPTS => {
                return Err(SandboxError::SnapshotReconciliationRequired {
                    retained_sandbox: None,
                });
            }
            BackendSnapshotRecovery::InProgress => {
                tokio::time::sleep(RECOVERY_INTERVAL).await;
            }
        }
    }
    Err(SandboxError::SnapshotReconciliationRequired {
        retained_sandbox: None,
    })
}

pub(super) fn live_config(api_key: String, template: String) -> E2bAdapterConfig {
    E2bAdapterConfig::new(
        LIVE_BACKEND_ID,
        "https://api.e2b.app",
        api_key,
        HashMap::from([(
            "live".to_owned(),
            E2bProfile {
                template,
                allow_public_egress: false,
                denied_destinations: vec!["203.0.113.10/32".to_owned()],
            },
        )]),
        600,
    )
    .expect("live adapter config")
}

pub(super) fn sandbox_request(
    owner: ResourceOwner,
    snapshot_provider_ref: Option<ProviderRef>,
) -> BackendCreateSandboxRequest {
    BackendCreateSandboxRequest {
        network: sandbox_interface::SandboxNetworkPolicy::Open,
        sandbox_id: SandboxId::new(),
        operation_id: OperationId::new(),
        owner,
        consumer: SandboxConsumer::Runtime,
        lifetime: sandbox_interface::SandboxLifetime::IdleAutoPause,
        deployment_id: "live-test".to_owned(),
        profile: "live".to_owned(),
        snapshot_provider_ref,
    }
}

pub(super) fn terminal_request(sandbox_provider_ref: ProviderRef) -> BackendTerminalCreateRequest {
    BackendTerminalCreateRequest {
        terminal_id: TerminalId::new(),
        operation_id: OperationId::new(),
        sandbox_provider_ref,
        cwd: None,
        provider_log_limit: LOG_LIMIT,
    }
}

pub(super) async fn wait_for_output(
    backend: &E2bSandboxBackend,
    sandbox: ProviderRef,
    terminal: &BackendTerminal,
    mut offset: u64,
    marker: &[u8],
) -> LiveResult<u64> {
    for _ in 0..40 {
        let output = backend
            .read_terminal(BackendOutputRequest {
                sandbox_provider_ref: sandbox.clone(),
                terminal_provider_ref: terminal.provider_ref.clone(),
                provider_log_path: terminal.provider_log_path.clone(),
                offset,
                max_bytes: 64 * 1024,
                provider_log_limit: LOG_LIMIT,
                wait: Duration::from_millis(250),
            })
            .await?;
        offset = output.next_offset;
        if output
            .bytes
            .windows(marker.len())
            .any(|window| window == marker)
        {
            return Ok(offset);
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    Err(io::Error::other("terminal marker was not observed").into())
}

#[tokio::test]
async fn ingress_client_does_not_follow_redirects() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind redirect endpoint");
    let address = listener.local_addr().expect("redirect endpoint address");
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept ingress probe");
        let mut request = Vec::new();
        while !request.windows(4).any(|window| window == b"\r\n\r\n") {
            let mut chunk = [0_u8; 1024];
            let read = stream.read(&mut chunk).expect("read ingress probe");
            assert_ne!(read, 0, "client closed before request headers");
            request.extend_from_slice(&chunk[..read]);
        }
        stream
            .write_all(
                b"HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:9/redirected\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            )
            .expect("write redirect response");
    });

    let response = ingress_client()
        .expect("ingress client")
        .get(format!("http://{address}/initial"))
        .send()
        .await
        .expect("redirect response must be returned without following it");
    server.join().expect("redirect server should finish");

    assert_eq!(response.status(), reqwest::StatusCode::FOUND);
}
