//! Shared local TLS fixture and typed consumer requests.

pub(super) mod fixture;
pub(super) mod requests;
pub(super) mod state;
pub(super) mod wire;

mod routes;
mod tls;

mod runtime_frames;
mod runtime_routes;
pub(super) mod runtime_state;
