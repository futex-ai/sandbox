//! Template-owned screen stack ensure, capability, and resize operations.

use std::time::Duration;

use sandbox_interface::{
    BackendEnsureScreenStackRequest, Error, Result, SCREEN_VIEWPORT_MAX_HEIGHT,
    SCREEN_VIEWPORT_MAX_PIXELS, SCREEN_VIEWPORT_MAX_WIDTH, SCREEN_VIEWPORT_MIN_HEIGHT,
    SCREEN_VIEWPORT_MIN_WIDTH, ScreenStackCapabilities, ScreenStackOutcome,
};
use serde::Deserialize;

use crate::process::{ProcessConnection, ProcessSplitOutput, SplitProcessCommand};

use super::{configured::E2bSandboxBackend, mapping};

const SCREEN_STDOUT_LIMIT: usize = 4096;
const SCREEN_STDERR_LIMIT: usize = 64 * 1024;
const SCREEN_ENSURE_TIMEOUT: Duration = Duration::from_secs(300);
const SCREEN_COMMAND_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Deserialize)]
struct CapabilityEnvelope {
    version: u32,
    features: CapabilityFeatures,
    viewport: CapabilityViewport,
}

#[derive(Deserialize)]
struct CapabilityFeatures {
    dynamic_resize: bool,
}

#[derive(Deserialize)]
struct CapabilityViewport {
    min_width: u32,
    max_width: u32,
    min_height: u32,
    max_height: u32,
    max_pixels: u64,
}

pub(super) async fn ensure(
    backend: &E2bSandboxBackend,
    request: BackendEnsureScreenStackRequest,
) -> Result<ScreenStackOutcome> {
    let connection = mapping::connection(backend, &request.sandbox_provider_ref).await?;
    let output = run(
        backend,
        connection.clone(),
        vec!["ensure".to_owned()],
        SCREEN_ENSURE_TIMEOUT,
    )
    .await?;
    if succeeded(&output) {
        let capabilities = detect_capabilities(backend, connection).await;
        return Ok(ScreenStackOutcome::Ready { capabilities });
    }
    Err(Error::BackendUnavailable {
        backend_id: backend.config.backend_id.clone(),
    })
}

async fn detect_capabilities(
    backend: &E2bSandboxBackend,
    connection: ProcessConnection,
) -> ScreenStackCapabilities {
    let Ok(output) = run(
        backend,
        connection,
        vec!["capabilities".to_owned()],
        SCREEN_COMMAND_TIMEOUT,
    )
    .await
    else {
        return ScreenStackCapabilities::default();
    };
    if !succeeded(&output) {
        return ScreenStackCapabilities::default();
    }
    let Ok(envelope) = serde_json::from_slice::<CapabilityEnvelope>(&output.stdout) else {
        return ScreenStackCapabilities::default();
    };
    ScreenStackCapabilities {
        dynamic_resize: envelope.version == 1
            && envelope.features.dynamic_resize
            && envelope.viewport.min_width == SCREEN_VIEWPORT_MIN_WIDTH
            && envelope.viewport.max_width == SCREEN_VIEWPORT_MAX_WIDTH
            && envelope.viewport.min_height == SCREEN_VIEWPORT_MIN_HEIGHT
            && envelope.viewport.max_height == SCREEN_VIEWPORT_MAX_HEIGHT
            && envelope.viewport.max_pixels == SCREEN_VIEWPORT_MAX_PIXELS,
    }
}

async fn run(
    backend: &E2bSandboxBackend,
    connection: ProcessConnection,
    args: Vec<String>,
    timeout: Duration,
) -> Result<ProcessSplitOutput> {
    backend
        .processes
        .run_split(
            connection,
            SplitProcessCommand {
                command: backend
                    .config
                    .runtime_conventions
                    .screen_helper_path()
                    .to_owned(),
                args,
                stdout_limit: SCREEN_STDOUT_LIMIT,
                stderr_limit: SCREEN_STDERR_LIMIT,
                deadline: timeout,
            },
        )
        .await
}

fn succeeded(output: &ProcessSplitOutput) -> bool {
    output.exited
        && output.exit_code == Some(0)
        && !output.stdout_overflowed
        && !output.stderr_overflowed
}
