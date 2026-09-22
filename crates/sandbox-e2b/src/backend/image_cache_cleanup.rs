//! Descriptor-relative removal of workload-owned image caches.

use std::time::Duration;

use crate::{
    process::{ProcessCommand, ProcessOutputCapture},
    trusted_python,
};

const CLEANER: &str = include_str!("helpers/image_cache_cleanup.py");

pub(super) fn command() -> ProcessCommand {
    command_for_paths(std::iter::empty())
}

fn command_for_paths(paths: impl IntoIterator<Item = String>) -> ProcessCommand {
    ProcessCommand {
        command: trusted_python::EXECUTABLE.to_owned(),
        args: trusted_python::command_args(CLEANER, paths),
        cwd: None,
        output_capture: ProcessOutputCapture::HardLimit { max_bytes: 4096 },
        timeout: Duration::from_secs(300),
        read_only: false,
    }
}

#[cfg(test)]
pub(super) fn test_command(paths: impl IntoIterator<Item = String>) -> ProcessCommand {
    command_for_paths(paths)
}
