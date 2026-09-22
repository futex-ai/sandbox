//! Shared backend conformance coverage for the E2B adapter.

use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use sandbox_interface::conformance::exercise_backend;
use unimock::{MockFn, Unimock, matching};

use crate::{
    ControlSandbox, ControlSandboxReadAccess, ControlSandboxState, ControlSnapshot,
    E2bControlApiMock, SandboxMetadata, backend::configured::E2bSandboxBackend,
};

use super::super::backend_conformance_process;
use super::fixtures::{access, config};

#[tokio::test]
async fn e2b_adapter_satisfies_the_shared_conformance_harness() {
    let create_index = Arc::new(AtomicUsize::new(0));
    let created_sandboxes = Arc::new(Mutex::new(Vec::<(SandboxMetadata, String)>::new()));
    let created_snapshots = Arc::new(Mutex::new(HashMap::<(String, String), String>::new()));
    let control = Unimock::new((
        E2bControlApiMock::list_sandboxes
            .each_call(matching!(_))
            .answers_arc({
                let created_sandboxes = created_sandboxes.clone();
                Arc::new(move |_, metadata| {
                    let sandboxes = created_sandboxes
                        .lock()
                        .expect("created sandbox lock")
                        .iter()
                        .filter(|(created_metadata, _)| {
                            metadata.iter().all(|(key, value)| {
                                created_metadata
                                    .get(key)
                                    .is_some_and(|created| created == value)
                            })
                        })
                        .map(|(created_metadata, sandbox_id)| ControlSandbox {
                            sandbox_id: sandbox_id.clone(),
                            state: ControlSandboxState::Running,
                            metadata: created_metadata.clone(),
                        })
                        .collect();
                    Ok(sandboxes)
                })
            }),
        E2bControlApiMock::create_sandbox
            .each_call(matching!(_))
            .answers_arc({
                let create_index = create_index.clone();
                let created_sandboxes = created_sandboxes.clone();
                Arc::new(move |_, request| {
                    let sandbox_id = match create_index.fetch_add(1, Ordering::Relaxed) {
                        0 => "one-shot",
                        1 => "source",
                        2 => "restore-one",
                        3 => "restore-two",
                        4 => "image-source",
                        _ => "failed-image-source",
                    };
                    created_sandboxes
                        .lock()
                        .expect("created sandbox lock")
                        .push((request.metadata, sandbox_id.to_owned()));
                    Ok(access(sandbox_id))
                })
            }),
        E2bControlApiMock::get_sandbox
            .each_call(matching!("source"))
            .answers(&|_, _| {
                Ok(ControlSandbox {
                    sandbox_id: "source".to_owned(),
                    state: ControlSandboxState::Running,
                    metadata: Default::default(),
                })
            }),
        E2bControlApiMock::get_sandbox_read_access
            .next_call(matching!("source"))
            .returns(Ok(ControlSandboxReadAccess {
                sandbox_id: "source".to_owned(),
                domain: "e2b.app".to_owned(),
                envd_access_token: "call-local-token".to_owned(),
            })),
        E2bControlApiMock::pause_sandbox
            .each_call(matching!("source"))
            .answers(&|_, _| Ok(())),
        E2bControlApiMock::connect_sandbox
            .each_call(matching!(_))
            .answers(&|_, sandbox_id| Ok(access(sandbox_id))),
        E2bControlApiMock::list_snapshots
            .each_call(matching!(_, _))
            .answers_arc({
                let created_snapshots = created_snapshots.clone();
                Arc::new(move |_, sandbox_id, correlation_name| {
                    Ok(created_snapshots
                        .lock()
                        .expect("created snapshot lock")
                        .get(&(sandbox_id.to_owned(), correlation_name.to_owned()))
                        .map(|snapshot_id| {
                            vec![ControlSnapshot {
                                snapshot_id: snapshot_id.clone(),
                            }]
                        })
                        .unwrap_or_default())
                })
            }),
        E2bControlApiMock::create_snapshot
            .each_call(matching!(_, _))
            .answers_arc({
                let created_snapshots = created_snapshots.clone();
                Arc::new(move |_, sandbox_id, correlation_name| {
                    let snapshot_id = if sandbox_id == "source" {
                        "snapshot"
                    } else {
                        "realized-image"
                    };
                    created_snapshots
                        .lock()
                        .expect("created snapshot lock")
                        .insert(
                            (sandbox_id.to_owned(), correlation_name.to_owned()),
                            snapshot_id.to_owned(),
                        );
                    Ok(ControlSnapshot {
                        snapshot_id: snapshot_id.to_owned(),
                    })
                })
            }),
        E2bControlApiMock::get_snapshot
            .each_call(matching!("source", _, "snapshot"))
            .answers(&|_, _, _, _| {
                Ok(ControlSnapshot {
                    snapshot_id: "snapshot".to_owned(),
                })
            }),
        E2bControlApiMock::delete_snapshot
            .each_call(matching!(_))
            .answers(&|_, _| Ok(())),
        E2bControlApiMock::kill_sandbox
            .each_call(matching!(_))
            .answers(&|_, _| Ok(())),
    ));
    let process = backend_conformance_process::transport();
    let backend =
        E2bSandboxBackend::with_transports(config(), Arc::new(control), Arc::new(process));

    exercise_backend(&backend, "general")
        .await
        .expect("E2B adapter should conform");
}
