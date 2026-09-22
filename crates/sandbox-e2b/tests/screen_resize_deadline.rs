//! Local process smoke for the exact initialization supervisor command.

#![cfg(target_os = "linux")]

use std::collections::HashMap;
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use sandbox_e2b::{
    ControlSandboxReadAccess, E2bAdapterConfig, E2bControlApiMock, E2bProfile, E2bSandboxBackend,
    ProcessSplitOutput, ProcessTransportMock,
};
use sandbox_interface::{
    BackendResizeScreenStackRequest, Error, ProviderRef, SandboxBackend, ScreenViewportSize,
};
use unimock::{MockFn, Unimock, matching};

const HELPER: &str = r#"
import subprocess, sys
child = subprocess.Popen([sys.executable, '-c', '''
import pathlib, sys, time
pathlib.Path(sys.argv[1]).write_text('started')
time.sleep(1.5)
pathlib.Path(sys.argv[2]).write_text('late resize')
''', sys.argv[1], sys.argv[2]])
child.wait()
"#;

#[tokio::test]
async fn initialization_timeout_kills_the_helper_and_its_children_before_returning() {
    let directory = tempfile::tempdir().expect("fixture directory");
    let started = directory.path().join("started");
    let late_mutation = directory.path().join("late-mutation");
    let process_started = started.clone();
    let process_mutation = late_mutation.clone();
    let processes = Unimock::new(
        ProcessTransportMock::run_split
            .next_call(matching!(_, _))
            .answers_arc(Arc::new(move |_, _, command| {
                assert_eq!(command.command, "/usr/bin/timeout");
                assert_eq!(command.args[2], "/usr/local/bin/sandbox-screen");
                let output = Command::new(&command.command)
                    .args(&command.args[..2])
                    .args(["/usr/bin/python3", "-c", HELPER])
                    .arg(&process_started)
                    .arg(&process_mutation)
                    .output()
                    .expect("run actual supervisor with a fixture helper");
                assert!(!output.status.success());
                Ok(ProcessSplitOutput {
                    stdout: output.stdout,
                    stderr: output.stderr,
                    exit_code: Some(output.status.code().unwrap_or(-1)),
                    exited: output.status.code().is_some(),
                    ..ProcessSplitOutput::default()
                })
            })),
    );
    let control = Unimock::new(
        E2bControlApiMock::get_sandbox_read_access
            .next_call(matching!(_))
            .returns(Ok(ControlSandboxReadAccess {
                sandbox_id: "provider".to_owned(),
                domain: "e2b.app".to_owned(),
                envd_access_token: "test".to_owned(),
            })),
    );
    let config = E2bAdapterConfig::new(
        "e2b",
        "https://api.e2b.app",
        "test",
        HashMap::from([(
            "general".to_owned(),
            E2bProfile {
                template: "test".to_owned(),
                allow_public_egress: false,
                denied_destinations: vec!["203.0.113.10/32".to_owned()],
            },
        )]),
        600,
    )
    .expect("valid config");
    let backend =
        E2bSandboxBackend::with_transports(config, Arc::new(control), Arc::new(processes));
    let result = backend
        .resize_screen_stack(BackendResizeScreenStackRequest {
            sandbox_provider_ref: ProviderRef::new("provider"),
            viewport: ScreenViewportSize::new(704, 768).expect("viewport"),
            deadline_at: Some((SystemTime::now() + Duration::from_secs(4)).into()),
        })
        .await;
    assert!(matches!(result, Err(Error::BackendUnavailable { .. })));
    assert!(
        started.exists(),
        "fixture child must have started before expiry"
    );
    tokio::time::sleep(Duration::from_millis(1600)).await;
    assert!(
        !late_mutation.exists(),
        "a child escaped the supervisor's process group"
    );
}
