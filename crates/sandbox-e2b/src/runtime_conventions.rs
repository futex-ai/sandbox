//! Validated deployment-specific metadata, helper, and process names.

use std::path::{Component, Path};

use crate::error::{Error, Result};

const DEFAULT_METADATA_PREFIX: &str = "sandbox";
const DEFAULT_TERMINAL_TAG_PREFIX: &str = "sandbox-terminal-";
const DEFAULT_SCREEN_HELPER_PATH: &str = "/usr/local/bin/sandbox-screen";
const DEFAULT_IMAGE_HELPER_PROCESS_NAME: &str = "sandbox-helper";
const DEFAULT_IMAGE_AGENT_PROCESS_NAME: &str = "sandbox-agent";
const CONVENTION_MAX_BYTES: usize = 255;
const PROCESS_NAME_MAX_BYTES: usize = 15;

/// Deployment-specific names embedded in E2B metadata and template processes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct E2bRuntimeConventions {
    metadata_prefix: String,
    terminal_tag_prefix: String,
    screen_helper_path: String,
    image_helper_process_name: String,
    image_agent_process_name: String,
}

impl E2bRuntimeConventions {
    /// Validates conventions used to find resources created by one deployment.
    pub fn new(
        metadata_prefix: impl Into<String>,
        terminal_tag_prefix: impl Into<String>,
        screen_helper_path: impl Into<String>,
    ) -> Result<Self> {
        let conventions = Self {
            metadata_prefix: metadata_prefix.into(),
            terminal_tag_prefix: terminal_tag_prefix.into(),
            screen_helper_path: screen_helper_path.into(),
            image_helper_process_name: DEFAULT_IMAGE_HELPER_PROCESS_NAME.to_owned(),
            image_agent_process_name: DEFAULT_IMAGE_AGENT_PROCESS_NAME.to_owned(),
        };
        if !valid_metadata_prefix(&conventions.metadata_prefix)
            || !valid_terminal_tag_prefix(&conventions.terminal_tag_prefix)
            || !valid_absolute_path(&conventions.screen_helper_path)
        {
            return Err(Error::InvalidRequest);
        }
        Ok(conventions)
    }

    /// Replaces neutral image-cleanup process names with deployment values.
    pub fn with_image_process_names(
        mut self,
        helper: impl Into<String>,
        agent: impl Into<String>,
    ) -> Result<Self> {
        self.image_helper_process_name = helper.into();
        self.image_agent_process_name = agent.into();
        if !valid_process_name(&self.image_helper_process_name)
            || !valid_process_name(&self.image_agent_process_name)
        {
            return Err(Error::InvalidRequest);
        }
        Ok(self)
    }

    /// Prefix used for E2B resource-correlation metadata keys.
    #[must_use]
    pub fn metadata_prefix(&self) -> &str {
        &self.metadata_prefix
    }

    /// Prefix used for durable terminal process tags.
    #[must_use]
    pub fn terminal_tag_prefix(&self) -> &str {
        &self.terminal_tag_prefix
    }

    /// Absolute helper executable installed by compatible screen templates.
    #[must_use]
    pub fn screen_helper_path(&self) -> &str {
        &self.screen_helper_path
    }

    /// Exact helper process name stopped before an image snapshot.
    #[must_use]
    pub fn image_helper_process_name(&self) -> &str {
        &self.image_helper_process_name
    }

    /// Exact agent process name stopped before an image snapshot.
    #[must_use]
    pub fn image_agent_process_name(&self) -> &str {
        &self.image_agent_process_name
    }

    pub(crate) fn metadata_key(&self, suffix: &str) -> String {
        format!("{}_{suffix}", self.metadata_prefix)
    }
}

impl Default for E2bRuntimeConventions {
    fn default() -> Self {
        Self {
            metadata_prefix: DEFAULT_METADATA_PREFIX.to_owned(),
            terminal_tag_prefix: DEFAULT_TERMINAL_TAG_PREFIX.to_owned(),
            screen_helper_path: DEFAULT_SCREEN_HELPER_PATH.to_owned(),
            image_helper_process_name: DEFAULT_IMAGE_HELPER_PROCESS_NAME.to_owned(),
            image_agent_process_name: DEFAULT_IMAGE_AGENT_PROCESS_NAME.to_owned(),
        }
    }
}

fn valid_metadata_prefix(value: &str) -> bool {
    nonempty_canonical(value)
        && value.len() <= CONVENTION_MAX_BYTES
        && value.starts_with(|character: char| character.is_ascii_lowercase())
        && !value.ends_with('_')
        && !value.contains("__")
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn valid_terminal_tag_prefix(value: &str) -> bool {
    nonempty_canonical(value)
        && value.len() <= CONVENTION_MAX_BYTES
        && value.ends_with('-')
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn valid_absolute_path(value: &str) -> bool {
    nonempty_canonical(value)
        && value.len() <= CONVENTION_MAX_BYTES
        && value != "/"
        && !value.ends_with('/')
        && !value.contains("//")
        && !value
            .bytes()
            .any(|byte| byte.is_ascii_whitespace() || byte.is_ascii_control())
        && Path::new(value).is_absolute()
        && Path::new(value)
            .components()
            .all(|component| !matches!(component, Component::CurDir | Component::ParentDir))
}

fn valid_process_name(value: &str) -> bool {
    nonempty_canonical(value)
        && value.len() <= PROCESS_NAME_MAX_BYTES
        && value != "."
        && value != ".."
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn nonempty_canonical(value: &str) -> bool {
    !value.is_empty() && value.trim() == value
}
