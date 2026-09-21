//! Bounded cleanup for failed atomic regular-file replacement attempts.

use std::time::Duration;

use super::{
    connect::ConnectProcessTransport,
    types::{ProcessCommand, ProcessConnection, ProcessOutputCapture},
};

const CLEANUP_TIMEOUT: Duration = Duration::from_secs(30);

const CLEANER: &str = r#"import os
import sys
import time

failed = False
directory = None
try:
    root = sys.argv[1]
    path = sys.argv[2]
    staging = sys.argv[3]
    temporary = sys.argv[4]
    try:
        os.unlink(staging)
    except FileNotFoundError:
        pass
    except OSError:
        failed = True
    root_parts = [part for part in root.split('/') if part]
    path_parts = path.split('/')
    if root != '/' + '/'.join(root_parts):
        raise ValueError()
    if not path_parts or any(part in ('', '.', '..') for part in path_parts):
        raise ValueError()
    flags = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC
    directory = os.open('/', flags)
    for part in root_parts + path_parts[:-1]:
        child = os.open(part, flags, dir_fd=directory)
        os.close(directory)
        directory = child
    for delay in (0, 3):
        if delay:
            time.sleep(delay)
        try:
            os.unlink(temporary, dir_fd=directory)
        except FileNotFoundError:
            pass
        except OSError:
            failed = True
except (IndexError, OSError, ValueError):
    failed = True
finally:
    if directory is not None:
        os.close(directory)
raise SystemExit(1 if failed else 0)
"#;

pub(super) async fn cleanup(
    transport: &ConnectProcessTransport,
    connection: ProcessConnection,
    root: String,
    path: String,
    staging_path: String,
    temporary_name: String,
) {
    let result = transport
        .run_helper(
            connection,
            command(root, path, staging_path, temporary_name),
            CLEANUP_TIMEOUT,
        )
        .await;
    if !matches!(result, Ok(output) if output.exit_code == Some(0)) {
        tracing::debug!(event = "e2b_regular_file_cleanup_unconfirmed");
    }
}

fn command(
    root: String,
    path: String,
    staging_path: String,
    temporary_name: String,
) -> ProcessCommand {
    ProcessCommand {
        command: "/usr/bin/python3".to_owned(),
        args: vec![
            "-c".to_owned(),
            CLEANER.to_owned(),
            root,
            path,
            staging_path,
            temporary_name,
        ],
        cwd: None,
        output_capture: ProcessOutputCapture::HardLimit { max_bytes: 4096 },
        timeout: CLEANUP_TIMEOUT,
        read_only: false,
    }
}

#[cfg(test)]
#[path = "_tests_/regular_file_cleanup_tests.rs"]
mod regular_file_cleanup_tests;
