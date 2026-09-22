//! Provider-neutral screen stack requests, capabilities, and viewport values.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize};

use crate::{Error, ProviderRef, ResourceOwner, Result, SandboxId};

/// Minimum supported screen viewport width in CSS pixels.
pub const SCREEN_VIEWPORT_MIN_WIDTH: u32 = 320;
/// Maximum supported screen viewport width in CSS pixels.
pub const SCREEN_VIEWPORT_MAX_WIDTH: u32 = 3840;
/// Minimum supported screen viewport height in CSS pixels.
pub const SCREEN_VIEWPORT_MIN_HEIGHT: u32 = 240;
/// Maximum supported screen viewport height in CSS pixels.
pub const SCREEN_VIEWPORT_MAX_HEIGHT: u32 = 2160;
/// Maximum supported screen viewport area in CSS pixels.
pub const SCREEN_VIEWPORT_MAX_PIXELS: u64 = 8_294_400;

/// Exact validated screen viewport in CSS pixels.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct ScreenViewportSize {
    /// Exact viewport width.
    width: u32,
    /// Exact viewport height.
    height: u32,
}

impl<'de> Deserialize<'de> for ScreenViewportSize {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawViewport {
            width: u32,
            height: u32,
        }

        let raw = RawViewport::deserialize(deserializer)?;
        match Self::new(raw.width, raw.height) {
            Ok(viewport) => Ok(viewport),
            Err(error) => Err(serde::de::Error::custom(error)),
        }
    }
}

impl ScreenViewportSize {
    /// Validates and preserves one exact viewport pair without clamping.
    pub fn new(width: u32, height: u32) -> Result<Self> {
        let pixels = u64::from(width) * u64::from(height);
        if !(SCREEN_VIEWPORT_MIN_WIDTH..=SCREEN_VIEWPORT_MAX_WIDTH).contains(&width)
            || !(SCREEN_VIEWPORT_MIN_HEIGHT..=SCREEN_VIEWPORT_MAX_HEIGHT).contains(&height)
            || pixels > SCREEN_VIEWPORT_MAX_PIXELS
        {
            return Err(Error::InvalidScreenViewport { width, height });
        }
        Ok(Self { width, height })
    }

    /// Returns the exact validated viewport width.
    #[must_use]
    pub const fn width(self) -> u32 {
        self.width
    }

    /// Returns the exact validated viewport height.
    #[must_use]
    pub const fn height(self) -> u32 {
        self.height
    }
}

/// Capabilities detected from one ready screen stack.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ScreenStackCapabilities {
    /// Whether the template can apply exact dynamic viewport sizes.
    pub dynamic_resize: bool,
}

/// Request to ensure the screen stack in one owned sandbox.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EnsureScreenStackRequest {
    /// Caller resource owner.
    pub owner: ResourceOwner,
    /// Stable sandbox handle.
    pub sandbox_id: SandboxId,
}

/// Provider request to ensure a screen-capable sandbox's stack.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendEnsureScreenStackRequest {
    /// Source provider sandbox reference.
    pub sandbox_provider_ref: ProviderRef,
}

/// Request to resize the screen stack in one owned sandbox.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResizeScreenStackRequest {
    /// Caller resource owner.
    pub owner: ResourceOwner,
    /// Stable sandbox handle.
    pub sandbox_id: SandboxId,
    /// Exact validated viewport.
    pub viewport: ScreenViewportSize,
    /// Optional original deadline; bounded initialization must not resume a sandbox.
    pub deadline_at: Option<DateTime<Utc>>,
}

/// Provider request to resize a screen-capable sandbox's stack.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendResizeScreenStackRequest {
    /// Source provider sandbox reference.
    pub sandbox_provider_ref: ProviderRef,
    /// Exact validated viewport.
    pub viewport: ScreenViewportSize,
    /// Original initialization deadline, including credential lookup and execution.
    pub deadline_at: Option<DateTime<Utc>>,
}

/// Result of checking or ensuring one sandbox screen stack.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScreenStackOutcome {
    /// The sandbox profile does not declare screen support.
    Unsupported,
    /// Both screen bridges are ready with the detected helper capabilities.
    Ready {
        /// Conservatively detected template capabilities.
        capabilities: ScreenStackCapabilities,
    },
}

#[cfg(test)]
#[path = "_tests_/screen_stack_tests.rs"]
mod screen_stack_tests;
