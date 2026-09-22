//! Typed E2B control API client and status mapping.

use async_trait::async_trait;

use crate::error::{Error, Result};

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

const PRIVATE_NETWORK_DENIES: &[&str] = &[
    "10.0.0.0/8",
    "100.64.0.0/10",
    "127.0.0.0/8",
    "169.254.0.0/16",
    "172.16.0.0/12",
    "192.168.0.0/16",
    "224.0.0.0/4",
    "::1/128",
    "fc00::/7",
    "fe80::/10",
];

/// Reqwest-backed E2B control API implementation.
pub struct ReqwestE2bControlApi {
    pub(super) transport: std::sync::Arc<dyn E2bHttpTransport>,
    pub(super) sandbox_domain: String,
    pub(super) idle_timeout_seconds: u32,
}

#[async_trait]
impl E2bControlApi for ReqwestE2bControlApi {
    async fn list_sandboxes(&self, metadata: SandboxMetadata) -> Result<Vec<ControlSandbox>> {
        pagination::list_sandboxes(self, metadata).await
    }

    async fn create_sandbox(&self, request: ControlCreateSandbox) -> Result<ControlSandboxAccess> {
        let mut denies = PRIVATE_NETWORK_DENIES
            .iter()
            .map(|value| (*value).to_owned())
            .collect::<Vec<_>>();
        denies.extend(request.denied_destinations);
        denies.sort();
        denies.dedup();
        let body = encode(&CreateSandboxBody {
            template_id: request.template_id,
            metadata: request.metadata,
            secure: true,
            allow_internet_access: request.allow_public_egress,
            network: NetworkBody {
                allow_public_traffic: false,
                deny_out: denies,
            },
            auto_pause: true,
            auto_pause_memory: true,
            auto_resume: AutoResumeBody { enabled: false },
            timeout: request.idle_timeout_seconds,
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
            domain: self.sandbox_domain.clone(),
            envd_access_token: token,
        })
    }

    async fn connect_sandbox(&self, sandbox_id: &str) -> Result<ControlSandboxAccess> {
        let body = encode(&ConnectBody {
            timeout: self.idle_timeout_seconds,
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

    async fn pause_sandbox(&self, sandbox_id: &str) -> Result<()> {
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

#[cfg(test)]
#[path = "_tests_/control_client_tests.rs"]
mod control_client_tests;

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
