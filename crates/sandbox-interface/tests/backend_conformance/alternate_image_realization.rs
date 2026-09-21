//! Alternate-backend image-realization behavior for conformance coverage.

use sandbox_interface::{
    BackendRealizeImageRequest, BackendRealizedImage, Error, ProviderRef, Result,
    RetainedSandboxRef,
};

/// Realizes or rejects an image using deterministic alternate-backend behavior.
pub(super) fn realize(request: BackendRealizeImageRequest) -> Result<BackendRealizedImage> {
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
                provider_ref: ProviderRef::new("alternate-retained-source"),
            }),
        });
    }
    if request.setup_script == "infra-error" {
        return Err(Error::BackendUnavailable {
            backend_id: "alternate".to_owned(),
        });
    }
    if request.setup_script == "reconciliation-required" {
        return Err(Error::SnapshotReconciliationRequired {
            retained_sandbox: Some(RetainedSandboxRef {
                sandbox_id: request.sandbox_id,
                provider_ref: ProviderRef::new("alternate-reconciliation-source"),
            }),
        });
    }
    assert_eq!(
        request.setup_script,
        "test -f /tmp/sandbox-conformance-input.bin"
    );
    assert_eq!(request.input_files[0].bytes, b"input");
    assert_eq!(request.verify_commands, ["true"]);
    Ok(BackendRealizedImage {
        source_sandbox_cleanup_ref: ProviderRef::new("alternate-image-source"),
        image_provider_ref: ProviderRef::new("alternate-image"),
        size_bytes: 4096,
    })
}
