//! Runtime HTTPS budgets remain separate from short control requests.

use std::{sync::Arc, time::Duration};

use futures_util::StreamExt;
use sandbox_boat::{
    runtime::{
        api::{AttachRequest, BoatRuntimeApi},
        client::HttpsBoatRuntimeApi,
    },
    transport::{http::ReqwestBoatHttpTransport, settings::BoatHttpSettings},
};
use tokio::sync::{Notify, oneshot};

use super::support::{
    fixture::{self, Fixture},
    state::Override,
    wire::{Reply, Segment},
};

#[tokio::test]
async fn attach_can_outlive_the_control_whole_request_budget_with_verified_tls() {
    let fixture = Fixture::start().await;
    let gates = (0..4).map(|_| Arc::new(Notify::new())).collect::<Vec<_>>();
    let mut segments = vec![Segment {
        bytes: b"open".to_vec(),
        wait: None,
        sent: None,
    }];
    for (index, gate) in gates.iter().enumerate() {
        segments.push(Segment {
            bytes: vec![b'0' + index as u8],
            wait: Some(gate.clone()),
            sent: None,
        });
    }
    fixture.state.lock().unwrap().overrides.push_back(Override {
        method: "GET".to_owned(),
        path: "/api/v1/sandboxes/bx_abcdefgh/commands/1/stream?stdoutOffset=0&stderrOffset=0"
            .to_owned(),
        reply: Reply {
            status: 200,
            body: Vec::new(),
            headers: Vec::new(),
            stall: false,
            segments,
        },
    });
    let transport = ReqwestBoatHttpTransport::with_settings(
        fixture::config(&fixture.origin),
        BoatHttpSettings {
            connect_timeout: Duration::from_secs(1),
            read_timeout: Duration::from_secs(1),
            request_timeout: Duration::from_secs(1),
            ..Default::default()
        },
        Some(&fixture.ca),
    )
    .unwrap();
    let api = HttpsBoatRuntimeApi::new(Arc::new(transport));
    let mut stream = api
        .attach(AttachRequest {
            sandbox: "bx_abcdefgh".to_owned(),
            process: 1,
            stdout: 0,
            stderr: 0,
            timeout: Duration::from_secs(10),
        })
        .await
        .unwrap();
    assert_eq!(stream.next().await.unwrap().unwrap(), b"open");
    tokio::time::pause();
    let (stop, mut running) = oneshot::channel::<()>();
    let keeper = tokio::spawn(async move {
        loop {
            tokio::select! { _ = &mut running => break, _ = tokio::task::yield_now() => {} }
        }
    });
    for (index, gate) in gates.into_iter().enumerate() {
        tokio::time::advance(Duration::from_millis(400)).await;
        gate.notify_one();
        let item = stream.next().await;
        assert!(matches!(item, Some(Ok(bytes)) if bytes == vec![b'0' + index as u8]));
    }
    assert!(stream.next().await.is_none());
    stop.send(()).unwrap();
    keeper.await.unwrap();
    tokio::time::resume();
    assert_eq!(fixture.captured().len(), 1);
}
