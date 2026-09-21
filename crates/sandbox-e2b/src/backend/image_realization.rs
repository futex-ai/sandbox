//! Native E2B preparation of a caller-owned platform image source.

use std::time::Duration;

use sandbox_interface::{
    BackendPrepareImageRequest, BackendPreparedImage, Error, ProviderRef, RealizeImageFileInput,
    Result, RetainedSandboxRef, SandboxId,
};

use crate::process::{ProcessCommand, ProcessConnection, ProcessOutputCapture, ProcessRunOutput};

use super::{
    configured::E2bSandboxBackend,
    files,
    image_command_diagnostic::{ImagePhase, run_phase},
    mapping,
};

const HELPER_OUTPUT_LIMIT: usize = 4096;
const SIZE_MARKER: &str = "__SANDBOX_IMAGE_SIZE__=";
const QUIESCE_AND_SCRUB: &str = r#"set -eu
jobs -pr | xargs -r kill || true
stop_named_process() {
    sandbox_process_name="$1"
    pkill -TERM -x -- "$sandbox_process_name" || true
    sandbox_stop_attempt=0
    while pgrep -x -- "$sandbox_process_name" >/dev/null && [ "$sandbox_stop_attempt" -lt 20 ]; do
        sleep 0.1
        sandbox_stop_attempt=$((sandbox_stop_attempt + 1))
    done
    if pgrep -x -- "$sandbox_process_name" >/dev/null; then
        pkill -KILL -x -- "$sandbox_process_name" || true
        sandbox_stop_attempt=0
        while pgrep -x -- "$sandbox_process_name" >/dev/null && [ "$sandbox_stop_attempt" -lt 20 ]; do
            sleep 0.1
            sandbox_stop_attempt=$((sandbox_stop_attempt + 1))
        done
    fi
    ! pgrep -x -- "$sandbox_process_name" >/dev/null
}
for sandbox_process_name in "$@"; do stop_named_process "$sandbox_process_name"; done
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

pub(super) async fn prepare(
    backend: &E2bSandboxBackend,
    request: BackendPrepareImageRequest,
) -> Result<BackendPreparedImage> {
    if !request.owner.is_platform() {
        return Err(Error::PlatformOwnerRequired);
    }
    let sandbox_id = request.sandbox_id;
    let source_provider_ref = request.source_provider_ref.clone();
    match prepare_source(backend, request).await {
        Ok(size_bytes) => Ok(BackendPreparedImage {
            source_provider_ref,
            size_bytes,
        }),
        Err(error) => Err(with_retained_source(error, sandbox_id, source_provider_ref)),
    }
}

fn with_retained_source(error: Error, sandbox_id: SandboxId, provider_ref: ProviderRef) -> Error {
    let retained_sandbox = Some(RetainedSandboxRef {
        sandbox_id,
        provider_ref,
    });
    match error {
        Error::ImageSetupFailed { command, .. } => Error::ImageSetupFailed {
            command,
            retained_sandbox,
        },
        Error::ImageVerificationFailed { index, command, .. } => Error::ImageVerificationFailed {
            index,
            command,
            retained_sandbox,
        },
        error => error,
    }
}

async fn prepare_source(
    backend: &E2bSandboxBackend,
    request: BackendPrepareImageRequest,
) -> Result<u64> {
    let BackendPrepareImageRequest {
        source_provider_ref,
        input_files,
        setup_script,
        verify_commands,
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
    observe_size(backend, connection).await
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
    if output.succeeded()
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
