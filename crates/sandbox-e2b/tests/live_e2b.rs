//! Credential-gated live E2B lifecycle smoke test.
#![cfg(feature = "live-e2b")]

#[path = "live_e2b/lifecycle.rs"]
mod lifecycle;
#[path = "live_e2b/port_ingress.rs"]
mod port_ingress;
#[path = "live_e2b/screen.rs"]
mod screen;
#[path = "live_e2b/support.rs"]
mod support;

use std::sync::Arc;

use sandbox_e2b::E2bSandboxBackend;
use sandbox_interface::{ResourceOwner, conformance::exercise_one_shot_lifetime};
use uuid::Uuid;

use self::support::{LiveResources, live_config};

#[tokio::test]
#[ignore = "requires E2B_API_KEY and incurs live provider usage"]
async fn live_e2b_snapshot_restore_and_terminal_lifecycle() {
    let api_key = std::env::var("E2B_API_KEY").expect("E2B_API_KEY is required");
    let template = std::env::var("E2B_TEMPLATE_ID").unwrap_or_else(|_| "base".to_owned());
    let config = live_config(api_key, template);
    let backend = Arc::new(E2bSandboxBackend::new(config.clone()).expect("live backend"));
    let owner = ResourceOwner::agent(Uuid::now_v7(), Uuid::now_v7());
    let mut resources = LiveResources::default();

    let outcome = lifecycle::run(backend.as_ref(), &config, owner, &mut resources).await;
    let cleanup = resources.cleanup(backend.as_ref()).await;

    if let Err(error) = outcome {
        panic!("live E2B lifecycle failed: {error}");
    }
    if let Err(error) = cleanup {
        panic!("live E2B cleanup failed: {error}");
    }
}

#[tokio::test]
#[ignore = "requires E2B_API_KEY and incurs live provider usage"]
async fn live_e2b_one_shot_create_and_destroy() {
    let api_key = std::env::var("E2B_API_KEY").expect("E2B_API_KEY is required");
    let template = std::env::var("E2B_TEMPLATE_ID").unwrap_or_else(|_| "base".to_owned());
    let backend = E2bSandboxBackend::new(live_config(api_key, template)).expect("live backend");

    exercise_one_shot_lifetime(&backend, "live")
        .await
        .expect("live E2B one-shot lifecycle");
}

#[tokio::test]
#[ignore = "requires E2B_API_KEY and incurs live provider usage"]
async fn live_e2b_private_port_ingress() {
    let api_key = std::env::var("E2B_API_KEY").expect("E2B_API_KEY is required");
    let config = live_config(api_key, "base".to_owned());
    let backend = E2bSandboxBackend::new(config).expect("live backend");
    let owner = ResourceOwner::agent(Uuid::now_v7(), Uuid::now_v7());
    let mut resources = LiveResources::default();

    let outcome = port_ingress::run(&backend, owner, &mut resources).await;
    let cleanup = resources.cleanup(&backend).await;

    if let Err(error) = outcome {
        panic!("live E2B port ingress failed: {error}");
    }
    if let Err(error) = cleanup {
        panic!("live E2B port ingress cleanup failed: {error}");
    }
}

#[tokio::test]
#[ignore = "requires E2B_API_KEY, E2B_SCREEN_TEMPLATE_ID, and live provider usage"]
async fn live_e2b_private_screen_bridges() {
    let api_key = std::env::var("E2B_API_KEY").expect("E2B_API_KEY is required");
    let template =
        std::env::var("E2B_SCREEN_TEMPLATE_ID").expect("E2B_SCREEN_TEMPLATE_ID is required");
    let config = live_config(api_key, template);
    let backend = E2bSandboxBackend::new(config).expect("live backend");
    let owner = ResourceOwner::agent(Uuid::now_v7(), Uuid::now_v7());
    let mut resources = LiveResources::default();

    let outcome = screen::run(&backend, owner, &mut resources).await;
    let cleanup = resources.cleanup(&backend).await;

    if let Err(error) = outcome {
        panic!("live E2B screen bridge test failed: {error}");
    }
    if let Err(error) = cleanup {
        panic!("live E2B cleanup failed: {error}");
    }
}
