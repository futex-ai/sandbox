//! Injected transports receive target-derived confirmation on each sandbox delete.

use std::sync::{Arc, Mutex};

use sandbox_boat::{
    control::{api::BoatControlApi, client::ReqwestBoatControlApi},
    transport::{
        http::{BoatHttpResponse, BoatHttpTransportMock},
        request::{BoatHttpMethod, BoatRoute},
    },
};
use serde_json::json;
use unimock::{MockFn, Unimock, matching};

#[tokio::test(start_paused = true)]
async fn sandbox_delete_requests_derive_confirmation_for_each_target_and_repetition() {
    let captured = Arc::new(Mutex::new(Vec::new()));
    let transport = Unimock::new(
        BoatHttpTransportMock::send
            .each_call(matching!(_))
            .answers_arc({
                let captured = captured.clone();
                Arc::new(move |_, request| {
                    let confirmation = request.delete_confirmation().map(str::to_owned);
                    let target = match request.route {
                        BoatRoute::Sandbox(id) => Some(id),
                        _ => None,
                    };
                    let mut captured = captured.lock().unwrap();
                    captured.push((request.method, target, confirmation));
                    let body = match captured.len() {
                        1 => deletion("bx_22222222", "processing"),
                        2 => deletion("bx_22222222", "completed"),
                        3 => Vec::new(),
                        _ => deletion("bx_33333333", "completed"),
                    };
                    Ok(BoatHttpResponse {
                        status: if captured.len() == 3 { 404 } else { 202 },
                        body,
                        retry_after: None,
                    })
                })
            }),
    );
    let control = ReqwestBoatControlApi::with_transport(Arc::new(transport));

    control.delete_sandbox("bx_22222222").await.unwrap();
    control.delete_sandbox("bx_22222222").await.unwrap();
    control.delete_sandbox("bx_33333333").await.unwrap();

    assert_eq!(
        *captured.lock().unwrap(),
        ["bx_22222222", "bx_22222222", "bx_22222222", "bx_33333333"].map(|id| (
            BoatHttpMethod::Delete,
            Some(id.to_owned()),
            Some(id.to_owned())
        ))
    );
}

fn deletion(target: &str, status: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "ok":true,"type":"deletion.operation","operation":{
            "id":"bdop_0123456789abcdef0123456789abcdef",
            "kind":"sandbox","targetId":target,"status":status
        }
    }))
    .unwrap()
}
