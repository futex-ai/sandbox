//! Native E2B composition for workspace-platform image realization.

use std::time::Duration;

use sandbox_interface::{
    BackendCreateSandboxRequest, BackendRealizeImageRequest, BackendRealizedImage, Error,
    ProviderRef, RealizeImageFileInput, Result, RetainedSandboxRef, SandboxId,
    SandboxNetworkPolicy,
};

use crate::process::{ProcessCommand, ProcessConnection, ProcessOutputCapture, ProcessRunOutput};

use super::{
    configured::E2bSandboxBackend,
    files,
    image_command_diagnostic::{ImagePhase, run_phase},
    image_snapshot, mapping, sandboxes,
};

const HELPER_OUTPUT_LIMIT: usize = 4096;
const SIZE_MARKER: &str = "__SANDBOX_IMAGE_SIZE__=";
const QUIESCE_AND_SCRUB: &str = r#"set -eu
jobs -pr | xargs -r kill || true
for sandbox_process_name in "$@"; do pkill -TERM -x -- "$sandbox_process_name" || true; done
if mountpoint -q /drives/me; then fusermount3 -u /drives/me || umount -l /drives/me; fi
pkill -TERM -f '[s]andbox-drive-' || true
rm -rf /tmp/sandbox-drive
sandbox_uid="$(id -u)"
sandbox_home="${HOME:?runtime home is required}"
find /tmp /var/tmp -mindepth 1 -maxdepth 1 ! -name '.*' -user "$sandbox_uid" -exec rm -rf -- {} +
rm -rf -- "$sandbox_home/.cache" "$sandbox_home/.npm/_cacache" "$sandbox_home/.cargo/registry/cache"
test ! -e "$sandbox_home/.git-credentials"
test ! -e "$sandbox_home/.netrc"
test ! -e "$sandbox_home/.ssh/id_rsa"
test -z "${GIT_ASKPASS:-}${SSH_ASKPASS:-}${SSH_AUTH_SOCK:-}${AWS_ACCESS_KEY_ID:-}${GOOGLE_APPLICATION_CREDENTIALS:-}"
"#;
const SIZE_COMMAND: &str = "du -sbx / 2>/dev/null | awk '{print \"__SANDBOX_IMAGE_SIZE__=\" $1}'";

pub(super) async fn realize(
    backend: &E2bSandboxBackend,
    request: BackendRealizeImageRequest,
) -> Result<BackendRealizedImage> {
    if !request.owner.is_platform() {
        return Err(Error::PlatformOwnerRequired);
    }
    let sandbox_id = request.sandbox_id;
    let source = sandboxes::create(
        backend,
        BackendCreateSandboxRequest {
            sandbox_id: request.sandbox_id,
            operation_id: request.operation_id,
            owner: request.owner,
            deployment_id: request.deployment_id.clone(),
            profile: request.profile.clone(),
            network: SandboxNetworkPolicy::Open,
            snapshot_provider_ref: request.parent_image_provider_ref.clone(),
        },
    )
    .await?;
    let source_provider_ref = source.provider_ref;
    match realize_in_sandbox(backend, request, source_provider_ref.clone()).await {
        Err(error) => match retained_failure(error, sandbox_id, source_provider_ref) {
            RetentionDecision::Retain(error) => Err(error),
            RetentionDecision::Destroy {
                error,
                provider_ref,
            } => {
                let _destroyed = sandboxes::destroy(backend, provider_ref).await;
                Err(error)
            }
        },
        Ok(image) => Ok(BackendRealizedImage {
            source_sandbox_cleanup_ref: source_provider_ref,
            image_provider_ref: image.provider_ref,
            size_bytes: image.size_bytes,
        }),
    }
}

fn retained_failure(
    error: Error,
    sandbox_id: SandboxId,
    provider_ref: ProviderRef,
) -> RetentionDecision {
    let retained_sandbox = Some(RetainedSandboxRef {
        sandbox_id,
        provider_ref: provider_ref.clone(),
    });
    match error {
        Error::ImageSetupFailed { command, .. } => {
            RetentionDecision::Retain(Error::ImageSetupFailed {
                command,
                retained_sandbox,
            })
        }
        Error::ImageVerificationFailed { index, command, .. } => {
            RetentionDecision::Retain(Error::ImageVerificationFailed {
                index,
                command,
                retained_sandbox,
            })
        }
        error => RetentionDecision::Destroy {
            error,
            provider_ref,
        },
    }
}

async fn realize_in_sandbox(
    backend: &E2bSandboxBackend,
    request: BackendRealizeImageRequest,
    source_provider_ref: ProviderRef,
) -> Result<RealizedProviderImage> {
    let BackendRealizeImageRequest {
        snapshot_id,
        operation_id,
        input_files,
        setup_script,
        verify_commands,
        correlation_name,
        ..
    } = request;
    let connection = mapping::connection(backend, &source_provider_ref).await?;
    write_input_files(backend, connection.clone(), input_files).await?;
    run_phase(
        backend,
        connection.clone(),
        format!("set -eu\n{setup_script}"),
        ImagePhase::Setup,
    )
    .await?;
    for (index, command) in verify_commands.iter().enumerate() {
        run_phase(
            backend,
            connection.clone(),
            format!("set -eu\n{command}"),
            ImagePhase::Verify(index),
        )
        .await?;
    }
    run_phase(
        backend,
        connection.clone(),
        scrub_command(backend),
        ImagePhase::Scrub,
    )
    .await?;
    let size_bytes = observe_size(backend, connection).await?;
    let image_provider_ref = image_snapshot::realize(
        backend,
        snapshot_id,
        operation_id,
        correlation_name,
        source_provider_ref,
    )
    .await?;
    Ok(RealizedProviderImage {
        provider_ref: image_provider_ref,
        size_bytes,
    })
}

fn scrub_command(backend: &E2bSandboxBackend) -> String {
    let conventions = backend.config.runtime_conventions();
    format!(
        "set -- {} {}\n{QUIESCE_AND_SCRUB}",
        conventions.image_helper_process_name(),
        conventions.image_agent_process_name(),
    )
}

async fn write_input_files(
    backend: &E2bSandboxBackend,
    connection: ProcessConnection,
    input_files: Vec<RealizeImageFileInput>,
) -> Result<()> {
    for input in input_files {
        files::write_to_connection(
            backend,
            connection.clone(),
            input.root,
            input.path,
            input.bytes,
        )
        .await?;
    }
    Ok(())
}

async fn observe_size(backend: &E2bSandboxBackend, connection: ProcessConnection) -> Result<u64> {
    let output = backend
        .processes
        .run(
            connection,
            ProcessCommand {
                command: "/bin/sh".to_owned(),
                args: vec!["-lc".to_owned(), SIZE_COMMAND.to_owned()],
                cwd: None,
                output_capture: ProcessOutputCapture::HardLimit {
                    max_bytes: HELPER_OUTPUT_LIMIT,
                },
                timeout: Duration::from_secs(300),
                read_only: false,
            },
        )
        .await?;
    parse_size(output)
}

fn parse_size(output: ProcessRunOutput) -> Result<u64> {
    let size = std::str::from_utf8(&output.bytes)
        .ok()
        .and_then(|value| {
            value
                .split_whitespace()
                .find_map(|token| token.strip_prefix(SIZE_MARKER))
        })
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0);
    if output.exited
        && output.exit_code == Some(0)
        && let Some(size) = size
    {
        return Ok(size);
    }
    tracing::warn!(
        exited = output.exited,
        exit_code = ?output.exit_code,
        output_bytes = output.bytes.len(),
        numeric_output = size.is_some(),
        "E2B image size observation did not produce a usable value"
    );
    Err(Error::ImageSizeUnavailable)
}

enum RetentionDecision {
    Retain(Error),
    Destroy {
        error: Error,
        provider_ref: ProviderRef,
    },
}

struct RealizedProviderImage {
    provider_ref: ProviderRef,
    size_bytes: u64,
}
