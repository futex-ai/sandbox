//! Substrate consumer classes carried by sandbox rows.

use serde::{Deserialize, Serialize};

/// Substrate consumer class of one sandbox row.
///
/// Environment, build, and fn-run sandboxes share the default [`Runtime`]
/// class. Browser session sandboxes carry the dedicated [`Browser`] class:
/// they are excluded from env listings and env quota counts, never accept
/// terminal creation, and reject snapshot capture and snapshot-sourced
/// creation so durable credential material can exist only as encrypted
/// profile checkpoints.
///
/// [`Runtime`]: SandboxConsumer::Runtime
/// [`Browser`]: SandboxConsumer::Browser
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxConsumer {
    /// Environment, build, and fn-run runtime consumer.
    #[default]
    Runtime,
    /// Browser session consumer.
    Browser,
}
