//! Mocked preparation state and cleanup observations for startup regressions.

use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, atomic::AtomicBool},
};

use futures_util::{StreamExt, stream};
use serde::Deserialize;
use serde_json::json;
use tokio::sync::Notify;
use unimock::{MockFn, Unimock, matching};

use crate::{
    config::{BoatAdapterConfig, BoatProfile},
    runtime::{
        api::{BoatRuntimeApiMock, DetachedProcess, HelperOutput},
        configured::BoatTrustedRuntime,
    },
    transport::runtime::BoatByteStream,
};

use super::{
    gated_api::{Gate, GatedApi, Point},
    timing_support::line,
};

#[derive(Default)]
pub(super) struct State {
    pub(super) attempt: String,
    pub(super) actions: Vec<String>,
    pub(super) prepared: bool,
    pub(super) stops: usize,
}

pub(super) struct Fixture {
    pub(super) runtime: Arc<BoatTrustedRuntime>,
    pub(super) state: Arc<Mutex<State>>,
    pub(super) gate: Arc<Gate>,
    pub(super) cleaned: Arc<Notify>,
    pub(super) detached: Arc<Notify>,
}

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Work {
    Unopened,
    Cancel,
    TimedOut,
}

#[derive(Deserialize)]
struct BundleIdentity {
    attempt: String,
}

struct DetachedSignal(Arc<Notify>);

impl Drop for DetachedSignal {
    fn drop(&mut self) {
        self.0.notify_one();
    }
}

pub(super) fn fixture(point: Point, fail: bool, work: Work) -> Fixture {
    let state = Arc::new(Mutex::new(State::default()));
    let cleaned = Arc::new(Notify::new());
    let detached = Arc::new(Notify::new());
    let gate = Arc::new(Gate {
        point,
        entered: Notify::new(),
        resume: Notify::new(),
        fail,
        armed: AtomicBool::new(true),
    });
    let helper_state = state.clone();
    let helper_cleaned = cleaned.clone();
    let setup = BoatRuntimeApiMock::helper
        .each_call(matching!(_, _, _))
        .answers_arc(Arc::new(move |_, _, command, _| {
            let action = command.rsplit(' ').next().unwrap();
            let mut state = helper_state.lock().unwrap();
            state.actions.push(action.to_owned());
            if action == "clean" {
                state.prepared = false;
                helper_cleaned.notify_one();
            }
            Ok(HelperOutput {
                exit_code: Some(0),
                stdout: String::new(),
                normal: true,
            })
        }));
    let api = if point == Point::Bootstrap {
        Unimock::new(setup)
    } else {
        let upload_state = state.clone();
        let setup = (
            setup,
            BoatRuntimeApiMock::upload
                .some_call(matching!(_, _, _, _))
                .answers_arc(Arc::new(move |_, _, _, bytes, _| {
                    let bundle: BundleIdentity = serde_json::from_slice(&bytes).unwrap();
                    let mut state = upload_state.lock().unwrap();
                    state.attempt = bundle.attempt;
                    state.prepared = true;
                    Ok(())
                })),
        );
        if work != Work::Unopened {
            let attach_state = state.clone();
            let stop_state = state.clone();
            let attached = detached.clone();
            let setup = (setup,
                BoatRuntimeApiMock::start.some_call(matching!(_, _, _)).returns(Ok(DetachedProcess {
                    process_id: 1,
                    launcher_pid: 11,
                })),
                BoatRuntimeApiMock::attach.some_call(matching!(_)).answers_arc(Arc::new(move |_, _| {
                    let attempt = attach_state.lock().unwrap().attempt.clone();
                    let mut initial = line(json!({"type":"attached","processId":1,"stdoutOffset":0,"stderrOffset":0}));
                    let hello = line(json!({"version":1,"attempt":attempt,"kind":"hello"}));
                    initial.extend(line(json!({"type":"stdout","data":std::str::from_utf8(&hello).unwrap(),"nextOffset":hello.len()})));
                    let stream: BoatByteStream = Box::pin(stream::iter([Ok(initial)]).chain(stream::pending()));
                    let attached = DetachedSignal(attached.clone());
                    let tail = stream::unfold((stream, attached), |(mut stream, attached)| async move {
                        let next = stream.next().await;
                        next.map(|value| (value, (stream, attached)))
                    });
                    Ok(Box::pin(tail))
                })),
            );
            if work == Work::TimedOut {
                Unimock::new((
                    setup,
                    BoatRuntimeApiMock::stop
                        .some_call(matching!(_, _, _))
                        .answers_arc(Arc::new(move |_, _, _, _| {
                            stop_state.lock().unwrap().stops += 1;
                            Ok(())
                        })),
                ))
            } else {
                Unimock::new(setup)
            }
        } else {
            Unimock::new(setup)
        }
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
    Fixture {
        runtime: Arc::new(BoatTrustedRuntime::new(
            config,
            Arc::new(GatedApi {
                inner: Arc::new(api),
                gate: gate.clone(),
            }),
        )),
        state,
        gate,
        cleaned,
        detached,
    }
}
