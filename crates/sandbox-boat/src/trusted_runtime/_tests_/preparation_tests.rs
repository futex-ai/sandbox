//! Preparation failures still attempt bounded private staging cleanup.

use std::{collections::BTreeMap, sync::Arc, time::Duration};

use sandbox_interface::{BackendStreamProcessRequest, Error, ProviderRef};
use tokio::sync::Notify;
use unimock::{MockFn, Unimock, matching};

use crate::{
    config::{BoatAdapterConfig, BoatProfile},
    error::Error as BoatError,
    runtime::{
        api::{BoatRuntimeApiMock, HelperOutput},
        configured::{BoatRuntime, BoatTrustedRuntime},
    },
};

#[tokio::test(start_paused = true)]
async fn failed_bootstrap_attempts_bounded_abandon_without_opening_a_workload() {
    for mode in 0..3 {
        let abandoned = Arc::new(Notify::new());
        let completed = abandoned.clone();
        let setup = (
            BoatRuntimeApiMock::helper
                .next_call(matching!(_, _, _))
                .answers_arc(Arc::new(move |_, _, _, _| {
                    if mode == 0 {
                        Err(BoatError::Unavailable)
                    } else {
                        Ok(HelperOutput {
                            exit_code: Some(0),
                            stdout: String::new(),
                            normal: mode == 2,
                        })
                    }
                })),
            BoatRuntimeApiMock::helper
                .next_call(matching!(_, _, _))
                .answers_arc(Arc::new(move |_, _, _, _| {
                    completed.notify_one();
                    Ok(HelperOutput {
                        exit_code: Some(0),
                        stdout: String::new(),
                        normal: true,
                    })
                })),
        );
        let api = if mode == 2 {
            Unimock::new((
                setup,
                BoatRuntimeApiMock::upload
                    .some_call(matching!(_, _, _, _))
                    .answers(&|_, _, _, _, _| Err(BoatError::Unavailable)),
            ))
        } else {
            Unimock::new(setup)
        };
        let config = BoatAdapterConfig::new(
            "boat",
            "https://boat.dev",
            "key",
            BTreeMap::from([(
                "general".to_owned(),
                BoatProfile {
                    provider_size: "default".to_owned(),
                    allow_public_egress: true,
                    denied_destinations: Vec::new(),
                },
            )]),
        )
        .unwrap();
        let runtime = BoatTrustedRuntime::new(config, Arc::new(api));
        let result = runtime
            .stream_process(BackendStreamProcessRequest {
                sandbox_provider_ref: ProviderRef::new("bx_abcdefgh"),
                command: "/bin/true".to_owned(),
                args: Vec::new(),
                cwd: None,
                envs: BTreeMap::new(),
                stdout_limit: 32,
                stderr_limit: 32,
                deadline: Duration::from_secs(10),
                idle_timeout: Duration::from_secs(1),
            })
            .await;
        assert!(matches!(result, Err(Error::BackendUnavailable { .. })));
        let cleanup = tokio::time::timeout(Duration::from_secs(1), abandoned.notified()).await;
        assert!(matches!(cleanup, Ok(())));
    }
}

#[tokio::test(start_paused = true)]
async fn canceled_bootstrap_attempts_bounded_abandon_without_opening_a_workload() {
    let abandoned = Arc::new(Notify::new());
    let completed = abandoned.clone();
    let api = Unimock::new((
        BoatRuntimeApiMock::helper
            .next_call(matching!(_, _, _))
            .answers(&|_, _, _, _| {
                Ok(HelperOutput {
                    exit_code: Some(0),
                    stdout: String::new(),
                    normal: true,
                })
            }),
        BoatRuntimeApiMock::helper
            .next_call(matching!(_, _, _))
            .answers_arc(Arc::new(move |_, _, _, _| {
                completed.notify_one();
                Ok(HelperOutput {
                    exit_code: Some(0),
                    stdout: String::new(),
                    normal: true,
                })
            })),
    ));
    let entered = Arc::new(Notify::new());
    let gate = super::preparation_support::GatedHelper {
        inner: Arc::new(api),
        entered: entered.clone(),
        release: Notify::new(),
        first: std::sync::atomic::AtomicBool::new(true),
    };
    let runtime = Arc::new(BoatTrustedRuntime::new(
        super::preparation_support::config(),
        Arc::new(gate),
    ));
    let pending_runtime = runtime.clone();
    let pending = tokio::spawn(async move {
        pending_runtime
            .stream_process(super::preparation_support::request())
            .await
    });
    entered.notified().await;
    pending.abort();
    let _ = pending.await;
    let cleanup = tokio::time::timeout(Duration::from_secs(1), abandoned.notified()).await;
    assert!(matches!(cleanup, Ok(())));
}
