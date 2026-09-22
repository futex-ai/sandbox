//! Native E2B preparation of a caller-owned platform image source.

use std::time::Duration;

use sandbox_interface::{
    BackendPrepareImageRequest, BackendPreparedImage, Error, ProviderRef, RealizeImageFileInput,
    Result, RetainedSandboxRef, SandboxId,
};

use crate::process::{ProcessCommand, ProcessConnection, ProcessOutputCapture, ProcessRunOutput};

use super::{
    configured::E2bSandboxBackend,
    files, image_cache_cleanup,
    image_command_diagnostic::{ImagePhase, run_phase, run_process_phase},
    mapping,
};

const HELPER_OUTPUT_LIMIT: usize = 4096;
const SIZE_MARKER: &str = "__SANDBOX_IMAGE_SIZE__=";
const TRUSTED_PROCESS_USER: &str = "root";
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
stop_sandbox_drive_helpers() {
    pkill -TERM -f -- '[s]andbox-drive-' || true
    sandbox_drive_stop_attempt=0
    while pgrep -f -- '[s]andbox-drive-' >/dev/null && [ "$sandbox_drive_stop_attempt" -lt 20 ]; do
        sleep 0.1
        sandbox_drive_stop_attempt=$((sandbox_drive_stop_attempt + 1))
    done
    if pgrep -f -- '[s]andbox-drive-' >/dev/null; then
        pkill -KILL -f -- '[s]andbox-drive-' || true
        sandbox_drive_stop_attempt=0
        while pgrep -f -- '[s]andbox-drive-' >/dev/null && [ "$sandbox_drive_stop_attempt" -lt 20 ]; do
            sleep 0.1
            sandbox_drive_stop_attempt=$((sandbox_drive_stop_attempt + 1))
        done
    fi
    ! pgrep -f -- '[s]andbox-drive-' >/dev/null
}
stop_sandbox_drive_helpers
rm -rf /tmp/sandbox-drive
sandbox_uid="$(id -u)"
sandbox_home="${HOME:?runtime home is required}"
find /tmp /var/tmp -mindepth 1 -maxdepth 1 ! -name '.*' -user "$sandbox_uid" -exec rm -rf -- {} +
test ! -e "$sandbox_home/.git-credentials"
test ! -e "$sandbox_home/.netrc"
test ! -e "$sandbox_home/.ssh/id_rsa"
test -z "${GIT_ASKPASS:-}${SSH_ASKPASS:-}${SSH_AUTH_SOCK:-}${AWS_ACCESS_KEY_ID:-}${GOOGLE_APPLICATION_CREDENTIALS:-}"
"#;
const SIZE_COMMAND: &str = r#"sandbox_size_output="$(du -sbx / 2>/dev/null)" || exit $?
sandbox_size="${sandbox_size_output%%[!0-9]*}"
test -n "$sandbox_size"
printf '__SANDBOX_IMAGE_SIZE__=%s\n' "$sandbox_size""#;

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
    for input in &input_files {
        files::validate_write_size(input.bytes.len())?;
        files::validate_transfer_paths(&input.root, &input.path)?;
    }
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
    run_process_phase(
        backend,
        connection.clone(),
        image_cache_cleanup::command(),
        ImagePhase::Scrub,
    )
    .await?;
    observe_size(backend, connection.with_user(TRUSTED_PROCESS_USER)).await
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
                args: vec!["-c".to_owned(), SIZE_COMMAND.to_owned()],
                cwd: None,
                envs: Default::default(),
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

#[cfg(test)]
#[path = "_tests_/image_safety_tests.rs"]
mod image_safety_tests;
