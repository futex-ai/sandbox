//! Boat lifecycle, saved snapshots, and advisory account capacity.

#![warn(unreachable_pub)]

pub mod backend;
pub mod catalogue;
pub mod config;
pub mod control;
pub mod error;
pub mod runtime;

mod files;
mod process;
pub mod transport;
mod trusted_runtime;

pub use backend::configured::BoatSandboxBackend;
pub use config::{BoatAdapterConfig, BoatProfile};
pub use error::Error as BoatAdapterError;
