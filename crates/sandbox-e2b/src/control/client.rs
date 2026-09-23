//! Typed E2B control API client and status mapping.

use async_trait::async_trait;

use crate::{
    error::{Error, Result},
    network,
};

use super::{
    E2bControlApi,
    helpers::{
        encode, ensure_sandbox_identity, map_access, map_state, path_segment,
        valid_provider_identity, valid_sandbox_identity,
    },
    http::{E2bHttpTransport, Method},
    pagination,
    types::{
        AutoResumeBody, ConnectBody, ControlCreateSandbox, ControlSandbox, ControlSandboxAccess,
        ControlSandboxReadAccess, ControlSandboxState, ControlSnapshot, CreateSandboxBody,
        NetworkBody, PauseBody, SandboxAccessBody, SandboxMetadata, SnapshotBody, SnapshotInfoBody,
    },
};

/// Reqwest-backed E2B control API implementation.
///
/// Sandbox creates revalidate typed network input before transport.
pub struct ReqwestE2bControlApi {
    pub(super) transport: std::sync::Arc<dyn E2bHttpTransport>,
    pub(super) sandbox_domain: String,
    pub(super) idle_timeout_seconds: u32,
    pub(super) lifetime_metadata_key: String,
}

impl ReqwestE2bControlApi {
    async fn connect_with_timeout(
        &self,
        sandbox_id: &str,
        timeout_seconds: u32,
    ) -> Result<ControlSandboxAccess> {
        if timeout_seconds == 0 {
            return Err(Error::InvalidRequest);
        }
        let body = encode(&ConnectBody {
            timeout: timeout_seconds,
        })?;
        let response: SandboxAccessBody = self
            .json(
                Method::Post,
                format!("/sandboxes/{}/connect", path_segment(sandbox_id)?),
                Some(body),
                &[200, 201],
                false,
            )
            .await?;
        ensure_sandbox_identity(sandbox_id, &response.sandbox_id)?;
        map_access(response, &self.sandbox_domain)
    }
}

#[async_trait]
impl E2bControlApi for ReqwestE2bControlApi {
    async fn list_sandboxes(&self, metadata: SandboxMetadata) -> Result<Vec<ControlSandbox>> {
        pagination::list_sandboxes(self, metadata).await
    }

    async fn create_sandbox(&self, request: ControlCreateSandbox) -> Result<ControlSandboxAccess> {
        let network = network::validate_control_create(
            request.allow_public_egress,
            request.allowed_destinations,
            request.denied_destinations,
        )?;
        let one_shot_timeout = match request.lifetime.one_shot_timeout_seconds() {
            Ok(timeout) => timeout,
            Err(_) => return Err(Error::InvalidRequest),
        };
        let (auto_pause, auto_pause_memory, timeout) = match one_shot_timeout {
            Some(timeout) => (false, false, timeout),
            None => (true, true, request.idle_timeout_seconds),
        };
        let body = encode(&CreateSandboxBody {
            template_id: request.template_id,
            metadata: request.metadata,
            secure: true,
            allow_internet_access: request.allow_public_egress,
            network: NetworkBody {
                allow_public_traffic: false,
                deny_out: network.deny_out,
                allow_out: network.allow_out,
            },
            auto_pause,
            auto_pause_memory,
            auto_resume: AutoResumeBody { enabled: false },
            timeout,
        })?;
        let response: SandboxAccessBody = self
            .json(
                Method::Post,
                "/sandboxes".to_owned(),
                Some(body),
                &[201],
                true,
            )
            .await?;
        if !valid_sandbox_identity(&response.sandbox_id) {
            return Err(Error::DeliveryAmbiguous);
        }
        match map_access(response, &self.sandbox_domain) {
            Ok(access) => Ok(access),
            Err(_) => Err(Error::DeliveryAmbiguous),
        }
    }

    async fn get_sandbox(&self, sandbox_id: &str) -> Result<ControlSandbox> {
        let response = self.sandbox_detail(sandbox_id).await?;
        ensure_sandbox_identity(sandbox_id, &response.sandbox_id)?;
        Ok(ControlSandbox {
            sandbox_id: response.sandbox_id,
            state: map_state(response.state),
            metadata: response.metadata,
        })
    }

    async fn get_sandbox_read_access(&self, sandbox_id: &str) -> Result<ControlSandboxReadAccess> {
        let response = self.sandbox_detail(sandbox_id).await?;
        ensure_sandbox_identity(sandbox_id, &response.sandbox_id)?;
        read_access(response, &self.sandbox_domain)
    }

    async fn connect_sandbox(&self, sandbox_id: &str) -> Result<ControlSandboxAccess> {
        self.connect_sandbox_with_timeout(sandbox_id, self.idle_timeout_seconds)
            .await
    }

    async fn connect_sandbox_with_timeout(
        &self,
        sandbox_id: &str,
        timeout_seconds: u32,
    ) -> Result<ControlSandboxAccess> {
        if timeout_seconds == 0 {
            return Err(Error::InvalidRequest);
        }
        let detail = self.sandbox_detail(sandbox_id).await?;
        ensure_sandbox_identity(sandbox_id, &detail.sandbox_id)?;
        if is_one_shot(&detail, &self.lifetime_metadata_key) {
            let access = read_access(detail, &self.sandbox_domain)?;
            return Ok(ControlSandboxAccess {
                sandbox_id: access.sandbox_id,
                domain: access.domain,
                envd_access_token: access.envd_access_token,
                traffic_access_token: None,
            });
        }
        self.connect_with_timeout(sandbox_id, timeout_seconds).await
    }

    async fn pause_sandbox(&self, sandbox_id: &str) -> Result<()> {
        let detail = self.sandbox_detail(sandbox_id).await?;
        ensure_sandbox_identity(sandbox_id, &detail.sandbox_id)?;
        if is_one_shot(&detail, &self.lifetime_metadata_key) {
            return Err(Error::Unavailable);
        }
        self.empty(
            Method::Post,
            format!("/sandboxes/{}/pause", path_segment(sandbox_id)?),
            Some(encode(&PauseBody { memory: true })?),
            &[204, 409],
        )
        .await
    }

    async fn kill_sandbox(&self, sandbox_id: &str) -> Result<()> {
        self.empty(
            Method::Delete,
            format!("/sandboxes/{}", path_segment(sandbox_id)?),
            None,
            &[204, 404],
        )
        .await
    }

    async fn create_snapshot(&self, sandbox_id: &str, name: &str) -> Result<ControlSnapshot> {
        if name.is_empty() {
            return Err(Error::InvalidRequest);
        }
        let response: SnapshotInfoBody = self
            .json(
                Method::Post,
                format!("/sandboxes/{}/snapshots", path_segment(sandbox_id)?),
                Some(encode(&SnapshotBody { name })?),
                &[201],
                true,
            )
            .await?;
        if !valid_provider_identity(&response.snapshot_id) {
            return Err(Error::DeliveryAmbiguous);
        }
        Ok(ControlSnapshot {
            snapshot_id: response.snapshot_id,
        })
    }

    async fn list_snapshots(&self, sandbox_id: &str, name: &str) -> Result<Vec<ControlSnapshot>> {
        pagination::list_snapshots(self, sandbox_id, name).await
    }

    async fn get_snapshot(
        &self,
        sandbox_id: &str,
        name: &str,
        snapshot_id: &str,
    ) -> Result<ControlSnapshot> {
        let rows = self.list_snapshots(sandbox_id, name).await?;
        rows.into_iter()
            .find(|snapshot| snapshot.snapshot_id == snapshot_id)
            .ok_or(Error::NotFound)
    }

    async fn delete_snapshot(&self, snapshot_id: &str) -> Result<()> {
        self.empty(
            Method::Delete,
            format!("/templates/{}", path_segment(snapshot_id)?),
            None,
            &[204, 404],
        )
        .await
    }
}

fn read_access(
    response: super::types::SandboxDetailBody,
    sandbox_domain: &str,
) -> Result<ControlSandboxReadAccess> {
    if map_state(response.state) != ControlSandboxState::Running {
        return Err(Error::Unavailable);
    }
    let Some(lifecycle) = response.lifecycle else {
        return Err(Error::Unavailable);
    };
    if lifecycle.auto_resume {
        return Err(Error::Unavailable);
    }
    let Some(token) = response
        .envd_access_token
        .filter(|token| !token.trim().is_empty())
    else {
        return Err(Error::Unavailable);
    };
    Ok(ControlSandboxReadAccess {
        sandbox_id: response.sandbox_id,
        domain: sandbox_domain.to_owned(),
        envd_access_token: token,
    })
}

fn is_one_shot(response: &super::types::SandboxDetailBody, lifetime_metadata_key: &str) -> bool {
    response
        .metadata
        .get(lifetime_metadata_key)
        .is_some_and(|value| value == "one_shot")
}

#[cfg(test)]
#[path = "_tests_/control_client_tests.rs"]
mod control_client_tests;

#[cfg(test)]
#[path = "_tests_/control_connect_timeout_tests.rs"]
mod control_connect_timeout_tests;
#[cfg(test)]
#[path = "_tests_/control_create_body_tests.rs"]
mod control_create_body_tests;

#[cfg(test)]
#[path = "_tests_/control_pagination_tests.rs"]
mod control_pagination_tests;

#[cfg(test)]
#[path = "_tests_/read_access_tests.rs"]
mod read_access_tests;

#[cfg(test)]
#[path = "_tests_/control_route_tests.rs"]
mod control_route_tests;

#[cfg(test)]
#[path = "_tests_/control_response_validation_tests.rs"]
mod control_response_validation_tests;

#[cfg(test)]
#[path = "_tests_/sandbox_lifetime_tests.rs"]
mod sandbox_lifetime_tests;
