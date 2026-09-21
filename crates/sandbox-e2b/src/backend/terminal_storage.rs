//! Non-following terminal directory creation and restored-state cleanup.

use std::time::Duration;

use crate::{
    process::{ProcessCommand, ProcessOutputCapture},
    trusted_python,
};

pub(super) const TERMINAL_LOG_DIRECTORY: &str = "/tmp/sandbox/terminals";
const SANDBOX_DRIVE_DIRECTORY: &str = "/tmp/sandbox-drive";
const DIRECTORY_CREATOR: &str = include_str!("helpers/terminal_log_directory.py");
const DIRECTORY_CLEANER: &str = include_str!("helpers/terminal_restore_cleanup.py");
const HELPER_TIMEOUT: Duration = Duration::from_secs(300);
const RESTORE_CLEANUP: &str = r#"set -eu
if mountpoint -q /drives/me; then
    fusermount3 -u /drives/me || umount -l /drives/me
fi
stop_sandbox_drive_helpers() {
    pkill -TERM -f -- '[s]andbox-drive-' || true
    sandbox_drive_stop_attempt=0
    while pgrep -f -- '[s]andbox-drive-' >/dev/null && [ "$sandbox_drive_stop_attempt" -lt 20 ]; do
        sleep 0.1
        sandbox_drive_stop_attempt=$((sandbox_drive_stop_attempt + 1))
    done
    if pgrep -f -- '[s]andbox-drive-' >/dev/null; then
        pkill -KILL -f -- '[s]andbox-drive-' || true
        sandbox_drive_stop_attempt=0
        while pgrep -f -- '[s]andbox-drive-' >/dev/null && [ "$sandbox_drive_stop_attempt" -lt 20 ]; do
            sleep 0.1
            sandbox_drive_stop_attempt=$((sandbox_drive_stop_attempt + 1))
        done
    fi
    ! pgrep -f -- '[s]andbox-drive-' >/dev/null
}
stop_sandbox_drive_helpers
exec /usr/bin/python3 -I -S -c "$1" "$2" "$3"
"#;

pub(super) fn create_directory_command() -> ProcessCommand {
    directory_command(TERMINAL_LOG_DIRECTORY)
}

fn directory_command(path: &str) -> ProcessCommand {
    python_command(DIRECTORY_CREATOR, [path])
}

pub(super) fn restore_cleanup_command() -> ProcessCommand {
    ProcessCommand {
        command: "/bin/sh".to_owned(),
        args: vec![
            "-c".to_owned(),
            RESTORE_CLEANUP.to_owned(),
            "sandbox-terminal-cleanup".to_owned(),
            DIRECTORY_CLEANER.to_owned(),
            SANDBOX_DRIVE_DIRECTORY.to_owned(),
            TERMINAL_LOG_DIRECTORY.to_owned(),
        ],
        cwd: None,
        output_capture: ProcessOutputCapture::HardLimit { max_bytes: 4096 },
        timeout: HELPER_TIMEOUT,
        read_only: false,
    }
}

#[cfg(test)]
fn cleanup_command(paths: &[&str]) -> ProcessCommand {
    python_command(DIRECTORY_CLEANER, paths.iter().copied())
}

fn python_command<'a>(
    script: &str,
    arguments: impl IntoIterator<Item = &'a str>,
) -> ProcessCommand {
    ProcessCommand {
        command: trusted_python::EXECUTABLE.to_owned(),
        args: trusted_python::command_args(script, arguments.into_iter().map(str::to_owned)),
        cwd: None,
        output_capture: ProcessOutputCapture::HardLimit { max_bytes: 4096 },
        timeout: HELPER_TIMEOUT,
        read_only: false,
    }
}

#[cfg(test)]
#[path = "_tests_/terminal_storage_tests.rs"]
mod terminal_storage_tests;
