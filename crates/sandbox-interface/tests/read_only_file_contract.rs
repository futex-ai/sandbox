//! Contract tests for lifecycle-invisible bounded regular-file reads.

use sandbox_interface::{
    ReadOnlyFileOutput, ReadOnlyFileRequest, ResourceOwner, SandboxId, SandboxReadOnly,
    SandboxReadOnlyMock,
};
use std::sync::Arc;
use unimock::{MockFn as _, Unimock, matching};
use uuid::Uuid;

#[tokio::test]
async fn trait_preserves_exact_coordinates_bytes_and_truncation() {
    let owner = ResourceOwner::agent(Uuid::now_v7(), Uuid::now_v7());
    let sandbox_id = SandboxId::new();
    let reader = Unimock::new(
        SandboxReadOnlyMock::read_only_file
            .next_call(matching!(_))
            .answers_arc(Arc::new(move |_, request| {
                assert_eq!(request.owner, owner);
                assert_eq!(request.sandbox_id, sandbox_id);
                assert_eq!(request.root, "/workspace/repo");
                assert_eq!(request.path, "src/lib.rs");
                assert_eq!(request.output_limit, 3);
                Ok(ReadOnlyFileOutput {
                    bytes: b"mod".to_vec(),
                    total_size: 7,
                    truncated: true,
                })
            })),
    );

    let output = reader
        .read_only_file(ReadOnlyFileRequest {
            owner,
            sandbox_id,
            root: "/workspace/repo".to_owned(),
            path: "src/lib.rs".to_owned(),
            output_limit: 3,
        })
        .await
        .expect("mocked bounded file read");

    assert_eq!(output.bytes, b"mod");
    assert_eq!(output.total_size, 7);
    assert!(output.truncated);
}
