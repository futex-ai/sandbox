//! Production client construction around an isolated TLS fake provider.

use std::sync::{Arc, Mutex};

use sandbox_boat::{
    BoatAdapterConfig, BoatProfile, BoatSandboxBackend,
    control::client::ReqwestBoatControlApi,
    runtime::{client::HttpsBoatRuntimeApi, configured::BoatTrustedRuntime},
    transport::{http::ReqwestBoatHttpTransport, settings::BoatHttpSettings},
};
use tokio::task::{JoinHandle, JoinSet};

use super::{routes, state::State, tls, wire};

pub struct Fixture {
    pub origin: String,
    pub ca: Vec<u8>,
    pub state: Arc<Mutex<State>>,
    pub backend: BoatSandboxBackend,
    task: JoinHandle<()>,
}

impl Fixture {
    pub async fn start() -> Self {
        let (listener, acceptor, ca) = tls::listener().await;
        let origin = format!(
            "https://127.0.0.1:{}",
            listener.local_addr().unwrap().port()
        );
        let state = Arc::new(Mutex::new(State::default()));
        let server_state = state.clone();
        let task = tokio::spawn(async move {
            let mut connections = JoinSet::new();
            loop {
                tokio::select! {
                    connection = listener.accept() => {
                        let (socket, _) = connection.unwrap();
                        let acceptor = acceptor.clone();
                        let state = server_state.clone();
                        connections.spawn(async move {
                            let Ok(mut stream) = acceptor.accept(socket).await else { return; };
                            let request = wire::read(&mut stream).await;
                            let reply = {
                                let mut state = state.lock().unwrap();
                                state.requests.push(request.clone());
                                routes::handle(&mut state, &request)
                            };
                            wire::write(&mut stream, reply).await;
                        });
                    }
                    result = connections.join_next(), if !connections.is_empty() => {
                        result.unwrap().unwrap();
                    }
                }
            }
        });
        let config = config(&origin);
        let transport = ReqwestBoatHttpTransport::with_settings(
            config.clone(),
            BoatHttpSettings::default(),
            Some(&ca),
        )
        .unwrap();
        let transport = Arc::new(transport);
        let backend = BoatSandboxBackend::with_clients(
            config.clone(),
            Arc::new(ReqwestBoatControlApi::with_transport(transport.clone())),
            Arc::new(BoatTrustedRuntime::new(
                config,
                Arc::new(HttpsBoatRuntimeApi::new(transport)),
            )),
        );
        Self {
            origin,
            ca,
            state,
            backend,
            task,
        }
    }

    pub fn control(&self, settings: BoatHttpSettings) -> ReqwestBoatControlApi {
        let transport =
            ReqwestBoatHttpTransport::with_settings(config(&self.origin), settings, Some(&self.ca))
                .unwrap();
        ReqwestBoatControlApi::with_transport(Arc::new(transport))
    }

    pub fn captured(&self) -> Vec<wire::CapturedRequest> {
        self.state.lock().unwrap().requests.clone()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}

pub fn config(origin: &str) -> BoatAdapterConfig {
    BoatAdapterConfig::new(
        "boat",
        origin,
        "fixture-key",
        [(
            "general".to_owned(),
            BoatProfile {
                provider_size: "default".to_owned(),
                allow_public_egress: true,
                denied_destinations: Vec::new(),
            },
        )]
        .into(),
    )
    .unwrap()
    .with_organization("personal")
    .unwrap()
}
