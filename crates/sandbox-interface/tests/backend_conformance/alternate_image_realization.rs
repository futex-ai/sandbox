//! Alternate-backend image-realization behavior for conformance coverage.

use sandbox_interface::{
    BackendPrepareImageRequest, BackendPreparedImage, Error, Result, RetainedSandboxRef,
};

/// Prepares or rejects an image source using deterministic alternate behavior.
pub(super) fn prepare(request: BackendPrepareImageRequest) -> Result<BackendPreparedImage> {
    assert!(request.owner.is_platform());
    if request.setup_script == "typed-without-details" {
        return Err(Error::ImageSetupFailed {
            command: None,
            retained_sandbox: None,
        });
    }
    if request.setup_script == "exit 7" {
        return Err(Error::ImageSetupFailed {
            command: None,
            retained_sandbox: Some(RetainedSandboxRef {
                sandbox_id: request.sandbox_id,
                provider_ref: request.source_provider_ref,
            }),
        });
    }
    if request.setup_script == "infra-error" {
        return Err(Error::BackendUnavailable {
            backend_id: "alternate".to_owned(),
        });
    }
    assert_eq!(
        request.setup_script,
        "test -f /tmp/sandbox-conformance-input.bin"
    );
    assert_eq!(request.input_files[0].bytes, b"input");
    assert_eq!(request.verify_commands, ["true"]);
    Ok(BackendPreparedImage {
        source_provider_ref: request.source_provider_ref,
        size_bytes: 4096,
    })
}
