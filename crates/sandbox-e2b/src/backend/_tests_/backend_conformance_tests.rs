//! Shared backend conformance coverage for the E2B adapter.

use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

use sandbox_interface::conformance::exercise_backend;
use unimock::{MockFn, Unimock, matching};

use crate::{
    ControlSandbox, ControlSandboxAccess, ControlSandboxReadAccess, ControlSandboxState,
    ControlSnapshot, E2bAdapterConfig, E2bControlApiMock, E2bProfile, ProcessFileChunk,
    ProcessInfo, ProcessRegularFileRequest, ProcessRegularFileWriteRequest, ProcessRunOutput,
    ProcessSelector, ProcessSplitOutput, ProcessTransportMock,
};

use super::configured::E2bSandboxBackend;

#[tokio::test]
async fn e2b_adapter_satisfies_the_shared_conformance_harness() {
    let create_index = Arc::new(AtomicUsize::new(0));
    let sandbox_list_index = Arc::new(AtomicUsize::new(0));
    let snapshot_created = Arc::new(AtomicBool::new(false));
    let snapshot_recovery_index = Arc::new(AtomicUsize::new(0));
    let control = Unimock::new((
        E2bControlApiMock::list_sandboxes
            .each_call(matching!(_))
            .answers_arc({
                let sandbox_list_index = sandbox_list_index.clone();
                Arc::new(move |_, _| {
                    if sandbox_list_index.fetch_add(1, Ordering::Relaxed) == 2 {
                        Ok(vec![ControlSandbox {
                            sandbox_id: "source".to_owned(),
                            state: ControlSandboxState::Running,
                            metadata: Default::default(),
                        }])
                    } else {
                        Ok(Vec::new())
                    }
                })
            }),
        E2bControlApiMock::create_sandbox
            .each_call(matching!(_))
            .answers_arc({
                let create_index = create_index.clone();
                Arc::new(move |_, _| {
                    let sandbox_id = match create_index.fetch_add(1, Ordering::Relaxed) {
                        0 => "source",
                        1 => "restore-one",
                        2 => "restore-two",
                        _ => "realize-source",
                    };
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
                let snapshot_created = snapshot_created.clone();
                let snapshot_recovery_index = snapshot_recovery_index.clone();
                Arc::new(move |_, sandbox_id, _| {
                    if sandbox_id == "source"
                        && snapshot_created.load(Ordering::Relaxed)
                        && snapshot_recovery_index.fetch_add(1, Ordering::Relaxed) > 0
                    {
                        Ok(vec![ControlSnapshot {
                            snapshot_id: "snapshot".to_owned(),
                        }])
                    } else {
                        Ok(Vec::new())
                    }
                })
            }),
        E2bControlApiMock::create_snapshot
            .each_call(matching!(_, _))
            .answers_arc({
                let snapshot_created = snapshot_created.clone();
                Arc::new(move |_, sandbox_id, _| {
                    let snapshot_id = if sandbox_id == "source" {
                        snapshot_created.store(true, Ordering::Relaxed);
                        "snapshot"
                    } else {
                        "realized-image"
                    };
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
    let active_terminal = Arc::new(Mutex::new(None));
    let process = Unimock::new((
        ProcessTransportMock::run
            .each_call(matching!(_, _))
            .answers(&|_, _, command| {
                let setup_fails = command
                    .args
                    .iter()
                    .any(|argument| argument.contains("exit 7"));
                let measures_image = command
                    .args
                    .iter()
                    .any(|argument| argument.contains("du -sbx"));
                let bytes = if measures_image {
                    b"__SANDBOX_IMAGE_SIZE__=8192\n".to_vec()
                } else {
                    Vec::new()
                };
                Ok(ProcessRunOutput {
                    bytes,
                    exit_code: Some(if setup_fails { 7 } else { 0 }),
                    exited: true,
                    output_truncated: false,
                })
            }),
        ProcessTransportMock::list
            .each_call(matching!(_))
            .answers_arc({
                let active_terminal = active_terminal.clone();
                Arc::new(move |_, _| {
                    Ok(active_terminal
                        .lock()
                        .expect("active terminal lock")
                        .clone()
                        .into_iter()
                        .collect())
                })
            }),
        ProcessTransportMock::start_pty
            .each_call(matching!(_, _))
            .answers_arc({
                let active_terminal = active_terminal.clone();
                Arc::new(move |_, _, request| {
                    let process = ProcessInfo {
                        pid: 9,
                        tag: Some(request.tag.clone()),
                    };
                    *active_terminal.lock().expect("active terminal lock") = Some(process.clone());
                    Ok(process)
                })
            }),
        ProcessTransportMock::send_input
            .each_call(matching!(_, _, _))
            .answers(&|_, _, selector, _| {
                assert!(matches!(selector, ProcessSelector::Tag(_)));
                Ok(())
            }),
        ProcessTransportMock::read_file
            .each_call(matching!(_, _, 0, 4096, _))
            .answers(&|_, _, _, _, _, _| {
                Ok(ProcessFileChunk {
                    bytes: b"conformance".to_vec(),
                    total_size: 11,
                })
            }),
        ProcessTransportMock::write_regular_file
            .each_call(matching!(_, _))
            .answers(&|_, _, request: ProcessRegularFileWriteRequest| {
                if request.path == "conformance.txt" {
                    assert_eq!(request.root, "/workspace");
                    assert_eq!(request.bytes, b"file-transfer");
                } else {
                    assert_eq!(request.bytes, b"input");
                }
                Ok(())
            }),
        ProcessTransportMock::read_regular_file
            .next_call(matching!(_, _))
            .answers(&|_, _, request: ProcessRegularFileRequest| {
                assert_eq!(request.root, "/workspace");
                assert_eq!(request.path, "conformance.txt");
                assert_eq!(request.offset, 0);
                assert_eq!(request.max_bytes, 4096);
                Ok(ProcessFileChunk {
                    bytes: b"file-transfer".to_vec(),
                    total_size: 13,
                })
            }),
        ProcessTransportMock::run_split
            .next_call(matching!(_, _))
            .answers(&|_, _, command| {
                assert_eq!(command.command, "conformance-command");
                assert_eq!(command.args, ["exact-argument"]);
                Ok(ProcessSplitOutput {
                    stdout: b"argv-direct".to_vec(),
                    stderr: b"separate-stderr".to_vec(),
                    exit_code: Some(0),
                    exited: true,
                    ..ProcessSplitOutput::default()
                })
            }),
        ProcessTransportMock::kill
            .each_call(matching!(_, _))
            .answers(&|_, _, selector| {
                assert!(matches!(selector, ProcessSelector::Tag(_)));
                Ok(())
            }),
    ));
    let backend =
        E2bSandboxBackend::with_transports(config(), Arc::new(control), Arc::new(process));

    exercise_backend(&backend, "general")
        .await
        .expect("E2B adapter should conform");
}

fn config() -> E2bAdapterConfig {
    E2bAdapterConfig::new(
        "configured-e2b",
        "https://api.e2b.app",
        "api-key",
        HashMap::from([(
            "general".to_owned(),
            E2bProfile {
                template: "base".to_owned(),
                allow_public_egress: false,
                denied_destinations: vec!["203.0.113.10/32".to_owned()],
            },
        )]),
        600,
    )
    .expect("valid adapter config")
}

fn access(sandbox_id: &str) -> ControlSandboxAccess {
    ControlSandboxAccess {
        sandbox_id: sandbox_id.to_owned(),
        domain: "e2b.app".to_owned(),
        envd_access_token: "call-local-token".to_owned(),
        traffic_access_token: "traffic-token".to_owned(),
    }
}
