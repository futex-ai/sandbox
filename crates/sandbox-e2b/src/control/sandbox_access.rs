//! Non-mutating one-shot access and existing lifetime-kind detection.

use crate::error::{Error, Result};

use super::{
    helpers::map_state,
    types::{ControlSandboxReadAccess, ControlSandboxState, SandboxDetailBody},
};

pub(super) fn read_access(
    response: SandboxDetailBody,
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

pub(super) fn is_one_shot(response: &SandboxDetailBody, lifetime_metadata_key: &str) -> bool {
    response
        .metadata
        .get(lifetime_metadata_key)
        .is_some_and(|value| value == "one_shot")
}
