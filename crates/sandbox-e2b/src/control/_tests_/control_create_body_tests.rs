//! Byte-exact E2B sandbox create request tests.

use std::{
    collections::VecDeque,
    net::{IpAddr, Ipv4Addr},
    sync::{Arc, Mutex},
};

use sandbox_interface::EgressDestination;
use unimock::{MockFn, Unimock, matching};

use crate::{
    ControlCreateSandbox, E2bControlApi, SandboxMetadata,
    control::http::{E2bHttpTransport, HttpRequest, HttpResponse, send},
};

use super::ReqwestE2bControlApi;

type RecordedRequests = Arc<Mutex<Vec<HttpRequest>>>;

const OPEN_BODY: &[u8] = br#"{"templateID":"template","metadata":{"sandbox_agent_id":"agent"},"secure":true,"allow_internet_access":false,"network":{"allowPublicTraffic":false,"denyOut":["10.0.0.0/8","100.64.0.0/10","127.0.0.0/8","169.254.0.0/16","172.16.0.0/12","192.168.0.0/16","203.0.113.0/24","224.0.0.0/4","::1/128","fc00::/7","fe80::/10"]},"autoPause":true,"autoPauseMemory":true,"autoResume":{"enabled":false},"timeout":600}"#;
const ALLOWLIST_BODY: &[u8] = br#"{"templateID":"template","metadata":{},"secure":true,"allow_internet_access":false,"network":{"allowPublicTraffic":false,"denyOut":["10.0.0.0/8","100.64.0.0/10","127.0.0.0/8","169.254.0.0/16","172.16.0.0/12","192.168.0.0/16","203.0.113.0/24","224.0.0.0/4","::1/128","fc00::/7","fe80::/10"],"allowOut":["192.0.2.10","198.51.100.0/24"]},"autoPause":true,"autoPauseMemory":true,"autoResume":{"enabled":false},"timeout":600}"#;

#[tokio::test]
async fn open_create_body_is_encoded_byte_for_byte() {
    let (client, requests) = recording_client();

    let access = client
        .create_sandbox(ControlCreateSandbox {
            template_id: "template".to_owned(),
            metadata: SandboxMetadata::from([("sandbox_agent_id".to_owned(), "agent".to_owned())]),
            allow_public_egress: false,
            denied_destinations: vec!["203.0.113.42/24".to_owned()],
            allowed_destinations: None,
            idle_timeout_seconds: 600,
        })
        .await
        .expect("create should decode");

    assert_eq!(access.sandbox_id, "provider-sandbox");
    assert_eq!(access.domain, "e2b.app");
    assert_eq!(access.traffic_access_token, "traffic-token");
    let body = only_body(&requests);
    assert_eq!(body, OPEN_BODY);
    assert!(!String::from_utf8_lossy(&body).contains("token"));
}

#[tokio::test]
async fn allowlist_create_body_is_encoded_byte_for_byte() {
    let (client, requests) = recording_client();

    client
        .create_sandbox(ControlCreateSandbox {
            template_id: "template".to_owned(),
            metadata: SandboxMetadata::new(),
            allow_public_egress: false,
            denied_destinations: vec!["203.0.113.0/24".to_owned()],
            allowed_destinations: Some(vec![
                EgressDestination::Ip("::ffff:192.0.2.10".parse().expect("mapped public IP")),
                EgressDestination::Cidr {
                    address: IpAddr::V4(Ipv4Addr::new(198, 51, 100, 42)),
                    prefix: 24,
                },
            ]),
            idle_timeout_seconds: 600,
        })
        .await
        .expect("allowlist create should decode");

    assert_eq!(only_body(&requests), ALLOWLIST_BODY);
}

#[tokio::test]
async fn unsafe_direct_allowlists_fail_before_transport() {
    let cases = vec![
        (
            false,
            vec![EgressDestination::Domain("api.example.com".to_owned())],
            vec!["203.0.113.0/24".to_owned()],
        ),
        (
            false,
            vec![EgressDestination::Ip(IpAddr::V4(Ipv4Addr::new(
                127, 0, 0, 1,
            )))],
            vec!["203.0.113.0/24".to_owned()],
        ),
        (
            false,
            vec![EgressDestination::Ip(IpAddr::V4(Ipv4Addr::new(
                203, 0, 113, 10,
            )))],
            vec!["203.0.113.0/24".to_owned()],
        ),
        (
            false,
            vec![EgressDestination::Ip(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1))); 65],
            vec!["203.0.113.0/24".to_owned()],
        ),
        (
            false,
            vec![EgressDestination::Cidr {
                address: IpAddr::V4(Ipv4Addr::new(192, 0, 2, 0)),
                prefix: 33,
            }],
            vec!["203.0.113.0/24".to_owned()],
        ),
        (
            true,
            vec![EgressDestination::Ip(IpAddr::V4(Ipv4Addr::new(
                192, 0, 2, 10,
            )))],
            vec!["203.0.113.0/24".to_owned()],
        ),
        (
            false,
            vec![EgressDestination::Ip(IpAddr::V4(Ipv4Addr::new(
                192, 0, 2, 10,
            )))],
            vec!["not-an-ip".to_owned()],
        ),
    ];

    for (allow_public_egress, allowed_destinations, denied_destinations) in cases {
        let client = client_without_transport();
        let result = client
            .create_sandbox(ControlCreateSandbox {
                template_id: "template".to_owned(),
                metadata: SandboxMetadata::new(),
                allow_public_egress,
                denied_destinations,
                allowed_destinations: Some(allowed_destinations),
                idle_timeout_seconds: 600,
            })
            .await;

        assert!(matches!(result, Err(crate::error::Error::InvalidRequest)));
    }
}

fn recording_client() -> (ReqwestE2bControlApi, RecordedRequests) {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let responses = Arc::new(Mutex::new(VecDeque::from([HttpResponse {
        status: 201,
        body: br#"{"sandboxID":"provider-sandbox","envdAccessToken":"token","trafficAccessToken":"traffic-token"}"#.to_vec(),
        next_token: None,
    }])));
    let transport: Arc<dyn E2bHttpTransport> =
        Arc::new(Unimock::new(send.next_call(matching!(_)).answers_arc({
            let requests = requests.clone();
            let responses = responses.clone();
            Arc::new(move |_, request| {
                requests.lock().expect("request lock").push(request);
                Ok(responses
                    .lock()
                    .expect("response lock")
                    .pop_front()
                    .expect("one response"))
            })
        })));
    (
        ReqwestE2bControlApi {
            transport,
            sandbox_domain: "e2b.app".to_owned(),
            idle_timeout_seconds: 600,
        },
        requests,
    )
}

fn client_without_transport() -> ReqwestE2bControlApi {
    ReqwestE2bControlApi {
        transport: Arc::new(Unimock::new(())),
        sandbox_domain: "e2b.app".to_owned(),
        idle_timeout_seconds: 600,
    }
}

fn only_body(requests: &RecordedRequests) -> Vec<u8> {
    requests.lock().expect("request lock")[0]
        .body
        .clone()
        .expect("create body")
}
