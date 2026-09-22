//! Serialization of metadata-only image failures.

use crate::ImageCommandFailure;

#[test]
fn failure_serialization_preserves_only_completion_and_capture_metadata() {
    for (exit_code, exited, output_bytes, output_truncated) in [
        (Some(7), true, 4096, true),
        (None, false, 0, false),
        (Some(-1), false, 12, false),
    ] {
        let failure = ImageCommandFailure {
            exit_code,
            exited,
            output_bytes,
            output_truncated,
        };
        let serialized = serde_json::to_value(&failure).expect("serializable metadata");
        assert_eq!(
            serialized,
            serde_json::json!({
                "exit_code": exit_code,
                "exited": exited,
                "output_bytes": output_bytes,
                "output_truncated": output_truncated,
            })
        );
        let restored: ImageCommandFailure =
            serde_json::from_value(serialized).expect("failure metadata should round-trip");
        assert_eq!(restored, failure);
    }
}
