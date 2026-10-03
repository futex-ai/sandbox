//! The streaming conformance probe requires both requested cwd and environment.

use std::{collections::BTreeMap, sync::Arc};

use futures_util::stream;
use unimock::{MockFn, Unimock, matching};

use crate::{Error, ProcessStreamEvent, ProcessStreamOutcome, ProviderRef, SandboxBackendMock};

use super::exercise;

#[tokio::test]
async fn backend_honoring_streaming_context_passes_the_probe() {
    exercise(
        &backend(b"/workspace\nstream-environment-map"),
        ProviderRef::new("provider"),
    )
    .await
    .expect("matching cwd, environment, and split output");
}

#[tokio::test]
async fn backend_ignoring_streaming_context_fails_the_probe() {
    let result = exercise(&backend(b"/home/user\n"), ProviderRef::new("provider")).await;

    assert!(matches!(result, Err(Error::Internal(_))));
}

fn backend(stdout: &'static [u8]) -> Unimock {
    Unimock::new(
        SandboxBackendMock::stream_process
            .next_call(matching!(_))
            .answers_arc(Arc::new(move |_, request| {
                assert_eq!(request.sandbox_provider_ref, ProviderRef::new("provider"));
                assert_eq!(request.command, "/bin/sh");
                assert_eq!(
                    request.args,
                    [
                        "-c",
                        "pwd; printf '%s' \"$SANDBOX_PROBE\"; printf '%s' 'stream-stderr' >&2"
                    ]
                );
                assert_eq!(request.cwd.as_deref(), Some("/workspace"));
                assert_eq!(
                    request.envs,
                    BTreeMap::from([(
                        "SANDBOX_PROBE".to_owned(),
                        "stream-environment-map".to_owned()
                    )])
                );
                Ok(Box::pin(stream::iter([
                    ProcessStreamEvent::Started { pid: 17 },
                    ProcessStreamEvent::Stdout(stdout.to_vec()),
                    ProcessStreamEvent::Stderr(b"stream-stderr".to_vec()),
                    ProcessStreamEvent::Exited {
                        exit_code: 0,
                        exited: true,
                    },
                    ProcessStreamEvent::Outcome(ProcessStreamOutcome::Completed),
                ])))
            })),
    )
}
