//! Mocked documented runtime boundary with deterministic provider delivery.

use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use futures_util::{StreamExt, stream};
use sandbox_interface::{BackendStreamProcessRequest, ProviderRef};
use serde_json::{Value, json};
use tokio::sync::{Notify, mpsc};
use unimock::{MockFn, Unimock, matching};

use crate::{
    config::{BoatAdapterConfig, BoatProfile},
    error::Result,
    runtime::{
        api::{BoatRuntimeApiMock, DetachedProcess, HelperOutput},
        configured::BoatTrustedRuntime,
    },
    transport::runtime::BoatByteStream,
};

pub(super) const ID: &str = "bx_abcdefgh";
#[derive(Clone, Copy)]
pub(super) enum Mode {
    Silent,
    Running,
    Ended,
    Channel,
    Late,
    UnknownDrop,
}
#[derive(Default)]
pub(super) struct State {
    pub(super) attempt: String,
    pub(super) actions: Vec<String>,
}
pub(super) struct Fixture {
    pub(super) runtime: BoatTrustedRuntime,
    pub(super) state: Arc<Mutex<State>>,
    pub(super) opened: Arc<Notify>,
    pub(super) terminated: Arc<Notify>,
    pub(super) input: mpsc::Sender<Result<Vec<u8>>>,
    pub(super) consumed: Arc<Notify>,
    pub(super) released: Arc<Notify>,
}

pub(super) fn fixture(mode: Mode) -> Fixture {
    let state = Arc::new(Mutex::new(State::default()));
    let opened = Arc::new(Notify::new());
    let terminated = Arc::new(Notify::new());
    let consumed = Arc::new(Notify::new());
    let delivered = consumed.clone();
    let released = Arc::new(Notify::new());
    let release = released.clone();
    let (input, receiver) = mpsc::channel(1);
    let receiver = Arc::new(Mutex::new(Some(receiver)));
    let upload_state = state.clone();
    let helper_state = state.clone();
    let attach_state = state.clone();
    let attach_opened = opened.clone();
    let killed = terminated.clone();
    let setup = (
        BoatRuntimeApiMock::helper
            .each_call(matching!(_, _, _))
            .answers_arc(Arc::new(move |_, _, command, _| {
                let action = command.rsplit(' ').next().unwrap().to_owned();
                helper_state.lock().unwrap().actions.push(action.clone());
                if action == "terminate" {
                    killed.notify_one();
                }
                if action == "release" {
                    release.notify_one();
                }
                Ok(HelperOutput {
                    exit_code: Some(0),
                    stdout: String::new(),
                    normal: true,
                })
            })),
        BoatRuntimeApiMock::upload
            .each_call(matching!(_, _, _, _))
            .answers_arc(Arc::new(move |_, _, path, bytes, _| {
                if path.ends_with("bundle.json") {
                    let bundle: Value = serde_json::from_slice(&bytes).unwrap();
                    upload_state.lock().unwrap().attempt =
                        bundle["attempt"].as_str().unwrap().to_owned();
                }
                Ok(())
            })),
        BoatRuntimeApiMock::start
            .some_call(matching!(_, _, _))
            .returns(Ok(DetachedProcess {
                process_id: 1,
                launcher_pid: 11,
            })),
        BoatRuntimeApiMock::attach
            .some_call(matching!(_))
            .answers_arc(Arc::new(move |_, _| {
                attach_opened.notify_one();
                let attempt = attach_state.lock().unwrap().attempt.clone();
                let initial = match mode {
                    Mode::Silent => Vec::new(),
                    Mode::Running | Mode::Channel => initial(&attempt, false),
                    Mode::Ended => initial(&attempt, true),
                    Mode::Late | Mode::UnknownDrop => before_start(&attempt),
                };
                let first = stream::iter((!initial.is_empty()).then_some(Ok(initial)));
                let tail: BoatByteStream =
                    if matches!(mode, Mode::Channel | Mode::Late | Mode::UnknownDrop) {
                        let receiver = receiver.lock().unwrap().take().unwrap();
                        let delivered = delivered.clone();
                        Box::pin(stream::unfold(
                            (receiver, delivered),
                            |(mut r, delivered)| async move {
                                r.recv().await.map(|v| {
                                    delivered.notify_one();
                                    (v, (r, delivered))
                                })
                            },
                        ))
                    } else {
                        Box::pin(stream::pending())
                    };
                Ok(Box::pin(first.chain(tail)))
            })),
    );
    let api = if matches!(mode, Mode::Ended | Mode::UnknownDrop) {
        Unimock::new(setup)
    } else {
        Unimock::new((
            setup,
            BoatRuntimeApiMock::stop
                .each_call(matching!(_, _, _))
                .answers(&|_, _, _, _| Ok(())),
        ))
    };
    let config = BoatAdapterConfig::new(
        "boat",
        "https://boat.dev",
        "test-key",
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
    Fixture {
        runtime: BoatTrustedRuntime::new(config, Arc::new(api)),
        state,
        opened,
        terminated,
        input,
        consumed,
        released,
    }
}

pub(super) fn request(deadline: u64, idle: u64) -> BackendStreamProcessRequest {
    BackendStreamProcessRequest {
        sandbox_provider_ref: ProviderRef::new(ID),
        command: "/bin/true".to_owned(),
        args: Vec::new(),
        cwd: None,
        envs: BTreeMap::new(),
        stdout_limit: 4096,
        stderr_limit: 4096,
        deadline: Duration::from_secs(deadline),
        idle_timeout: Duration::from_secs(idle),
    }
}

fn initial(attempt: &str, ended: bool) -> Vec<u8> {
    let mut inner = line(json!({"version":1,"attempt":attempt,"kind":"hello"}));
    inner.extend(line(
        json!({"version":1,"attempt":attempt,"kind":"started","pid":123}),
    ));
    if ended {
        inner.extend(line(
            json!({"version":1,"attempt":attempt,"kind":"exited","exit_code":0,"exited":true}),
        ));
        inner.extend(line(
            json!({"version":1,"attempt":attempt,"kind":"success"}),
        ));
    }
    let mut bytes =
        line(json!({"type":"attached","processId":1,"stdoutOffset":0,"stderrOffset":0}));
    bytes.extend(line(json!({"type":"stdout","data":std::str::from_utf8(&inner).unwrap(),"nextOffset":inner.len()})));
    bytes
}
pub(super) fn line(value: Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(&value).unwrap();
    bytes.push(b'\n');
    bytes
}
pub(super) fn start_offset(attempt: &str) -> usize {
    line(json!({"version":1,"attempt":attempt,"kind":"hello"})).len()
        + line(json!({"version":1,"attempt":attempt,"kind":"started","pid":123})).len()
}

fn before_start(attempt: &str) -> Vec<u8> {
    let inner = line(json!({"version":1,"attempt":attempt,"kind":"hello"}));
    let mut bytes =
        line(json!({"type":"attached","processId":1,"stdoutOffset":0,"stderrOffset":0}));
    bytes.extend(line(json!({"type":"stdout","data":std::str::from_utf8(&inner).unwrap(),"nextOffset":inner.len()})));
    bytes
}
