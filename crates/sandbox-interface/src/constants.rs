//! Shared code-owned request limits used across sandbox contract layers.

/// Maximum UTF-8 bytes accepted for a logical sandbox profile name.
pub const SANDBOX_PROFILE_MAX_BYTES: usize = 120;

/// Maximum normalized UTF-8 bytes retained for a failed image command.
pub const IMAGE_COMMAND_OUTPUT_MAX_BYTES: usize = 4096;

/// Maximum logical profiles configured for one sandbox backend.
pub const SANDBOX_PROFILE_MAX_ITEMS: usize = 32;

/// Returns whether a logical sandbox profile can be used as a bare environment reference.
#[must_use]
pub fn valid_sandbox_profile_name(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= SANDBOX_PROFILE_MAX_BYTES
        && (bytes[0].is_ascii_lowercase() || bytes[0].is_ascii_digit())
        && bytes.last().is_some_and(|byte| *byte != b'-')
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
}

/// Maximum regular-file payload accepted by one sandbox transfer.
pub const FILE_TRANSFER_MAX_BYTES: usize = 256 * 1024 * 1024;

/// Maximum UTF-8 byte length of a transfer root or relative path.
pub const FILE_TRANSFER_PATH_MAX_BYTES: usize = 4096;
